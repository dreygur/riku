//! `agent run`: one command in the app's directory with its live environment.
//!
//! Security model: the command runs as the riku user, so it can do anything
//! riku can, including commands a lower scope may not use. That is why `run`
//! needs the production scope (checked in `execute.rs`) and a confirm token
//! bound to the exact argv for every call. The command is executed directly,
//! not through a shell, with only PATH, HOME and LIVE_ENV in its environment.
//! Stdin is closed, each output stream is capped at `OUTPUT_CAP` bytes, and
//! the process group is killed when the command ends or at `RUN_TIMEOUT`.
//! It runs under the same rlimits as `riku run`: the server `RIKU_MAX_*`
//! values with the app's own put over them.

// Project rule 16 forbids `?`, and this lint asks for exactly that rewrite.
#![allow(clippy::question_mark)]

use std::collections::HashMap;
use std::io::{self, Read};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use nix::sys::signal::{killpg, Signal};
use nix::unistd::Pid;
use serde_json::json;

use crate::agent::confirm;
use crate::agent::types::AgentResponse;
use crate::config::RikuPaths;
use crate::util::resource_limits::ResourceLimits;
use crate::util::{existing_app, parse_settings};

const OUTPUT_CAP: usize = 64 * 1024;
const RUN_TIMEOUT: Duration = Duration::from_secs(300);

/// How long to keep reading output after the process group is killed. A
/// descendant that left the group with `setsid` can hold a pipe open forever,
/// so the run returns what it has by then.
const DRAIN_GRACE: Duration = Duration::from_secs(2);

/// Server variables the command keeps. Everything else, `SSH_ORIGINAL_COMMAND`
/// and riku's own settings included, is dropped before LIVE_ENV is added.
const KEPT_VARS: [&str; 2] = ["PATH", "HOME"];

struct Captured {
    text: String,
    truncated: bool,
}

struct RunOutcome {
    /// `None` when the run hit the time limit and was killed.
    status: Option<ExitStatus>,
    stdout: Captured,
    stderr: Captured,
}

#[derive(Default)]
struct Buffer {
    kept: Vec<u8>,
    truncated: bool,
}

/// One output stream read on its own thread into a shared buffer, so what
/// arrived can be taken even if the thread never sees end of file.
struct Stream {
    buffer: Arc<Mutex<Buffer>>,
    reader: JoinHandle<()>,
}

/// Reads the whole stream so the child never blocks on a full pipe, keeping
/// only the first `OUTPUT_CAP` bytes.
fn capture(mut source: impl Read + Send + 'static) -> Stream {
    let buffer = Arc::new(Mutex::new(Buffer::default()));
    let shared = Arc::clone(&buffer);
    let reader = thread::spawn(move || {
        let mut chunk = [0u8; 8192];
        loop {
            let read = match source.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(read) => read,
            };
            let mut buffer = match shared.lock() {
                Ok(buffer) => buffer,
                Err(_) => break,
            };
            let room = OUTPUT_CAP - buffer.kept.len();
            if read > room {
                buffer.truncated = true;
            }
            buffer.kept.extend_from_slice(&chunk[..read.min(room)]);
        }
    });
    Stream { buffer, reader }
}

/// Waits up to `deadline` for the reader, then takes what it read. A reader
/// still blocked by then marks the output truncated and is left behind.
fn collect(stream: Option<Stream>, deadline: Instant) -> Captured {
    let stream = match stream {
        Some(stream) => stream,
        None => {
            return Captured {
                text: String::new(),
                truncated: false,
            }
        }
    };
    while !stream.reader.is_finished() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(20));
    }
    let finished = stream.reader.is_finished();
    let captured = match stream.buffer.lock() {
        Ok(buffer) => Captured {
            text: String::from_utf8_lossy(&buffer.kept).into_owned(),
            truncated: buffer.truncated || !finished,
        },
        Err(_) => Captured {
            text: String::new(),
            truncated: true,
        },
    };
    captured
}

fn kill_group(child: &Child) {
    let _ = killpg(Pid::from_raw(child.id() as i32), Signal::SIGKILL);
}

/// Waits for the child, killing it at `limit`. Either way the whole process
/// group is killed afterwards, so background children cannot outlive the run
/// or hold its output pipes open.
fn wait_or_kill(child: &mut Child, limit: Duration) -> io::Result<Option<ExitStatus>> {
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                kill_group(child);
                return Ok(Some(status));
            }
            Ok(None) => {}
            Err(e) => {
                kill_group(child);
                return Err(e);
            }
        }
        if started.elapsed() >= limit {
            kill_group(child);
            let _ = child.wait();
            return Ok(None);
        }
        thread::sleep(Duration::from_millis(50));
    }
}

fn command_env(live_env: HashMap<String, String>) -> HashMap<String, String> {
    let mut env: HashMap<String, String> = KEPT_VARS
        .iter()
        .filter_map(|name| {
            std::env::var(name)
                .ok()
                .map(|value| (name.to_string(), value))
        })
        .collect();
    env.extend(live_env);
    env
}

/// Runs `cmd` with exactly `env`, under the server resource limits with the
/// `RIKU_MAX_*` values in `env` put over them.
fn run_captured(
    cmd: &[String],
    dir: &Path,
    env: &HashMap<String, String>,
    limit: Duration,
) -> io::Result<RunOutcome> {
    let limits = ResourceLimits::from_env().with_app_env(env);
    let mut command = Command::new(&cmd[0]);
    command
        .args(&cmd[1..])
        .current_dir(dir)
        .env_clear()
        .envs(env)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);
    unsafe {
        command.pre_exec(move || limits.apply());
    }
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(e) => return Err(e),
    };
    let stdout = child.stdout.take().map(capture);
    let stderr = child.stderr.take().map(capture);
    let status = match wait_or_kill(&mut child, limit) {
        Ok(status) => status,
        Err(e) => return Err(e),
    };
    let deadline = Instant::now() + DRAIN_GRACE;
    Ok(RunOutcome {
        status,
        stdout: collect(stdout, deadline),
        stderr: collect(stderr, deadline),
    })
}

fn outcome_response(cmd: &str, outcome: RunOutcome) -> AgentResponse {
    let exit_code = outcome.status.and_then(|status| status.code());
    let data = json!({
        "exit_code": exit_code,
        "stdout": outcome.stdout.text,
        "stderr": outcome.stderr.text,
        "truncated": outcome.stdout.truncated || outcome.stderr.truncated,
        "timed_out": outcome.status.is_none(),
    });
    if exit_code == Some(0) {
        return AgentResponse::success(data);
    }
    let reason = match outcome.status {
        None => format!("'{cmd}' ran past {}s and was killed", RUN_TIMEOUT.as_secs()),
        Some(status) => format!("'{cmd}' exited with {status}"),
    };
    let mut response = AgentResponse::error("COMMAND_FAILED", &reason);
    response.data = Some(data);
    response
}

/// Runs `cmd` in the app directory with LIVE_ENV and reports its output.
/// Every run needs a confirm token bound to the exact command.
pub fn cmd_agent_run(
    paths: &RikuPaths,
    state: &Path,
    app: &str,
    cmd: &[String],
    confirm_token: Option<&str>,
) -> AgentResponse {
    let app = match existing_app(app, &paths.app_root) {
        Ok(app) => app,
        Err(_) => {
            let message = format!("Application '{app}' not found");
            return AgentResponse::error("APP_NOT_FOUND", &message);
        }
    };
    if cmd.is_empty() {
        return AgentResponse::error("INVALID_PARAMETERS", "Missing command");
    }
    let bound = confirm::bound_action("run", cmd);
    if let Some(stop) = confirm::gate(state, "run", &bound, &app, confirm_token) {
        return stop;
    }
    let mut live_env = HashMap::new();
    let live_env_path = paths.env_root.join(&app).join("LIVE_ENV");
    if let Err(e) = parse_settings(&live_env_path, &mut live_env) {
        return AgentResponse::error("COMMAND_FAILED", &e.to_string());
    }
    let dir = paths.app_root.join(&app);
    match run_captured(cmd, &dir, &command_env(live_env), RUN_TIMEOUT) {
        Ok(outcome) => outcome_response(&cmd[0], outcome),
        Err(e) => {
            let message = format!("failed to start '{}': {e}", cmd[0]);
            AgentResponse::error("COMMAND_FAILED", &message)
        }
    }
}

#[cfg(test)]
#[path = "cmd_run_tests.rs"]
mod tests;
