//! `agent run`: one command in the app's directory with its live environment.
//!
//! Security model: the scope check (staging and up) happens in `execute.rs`.
//! The command is executed directly, never through a shell, so the agent's
//! words are argv, not shell text. Stdin is closed, each output stream is
//! capped at `OUTPUT_CAP` bytes, and the whole process group is killed after
//! `RUN_TIMEOUT`, so a run cannot hold the server's memory or a slot forever.

// Project rule 16 forbids `?`, and this lint asks for exactly that rewrite.
#![allow(clippy::question_mark)]

use std::collections::HashMap;
use std::io::{self, Read};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use nix::sys::signal::{killpg, Signal};
use nix::unistd::Pid;
use serde_json::json;

use crate::agent::types::AgentResponse;
use crate::config::RikuPaths;
use crate::util::{existing_app, parse_settings};

const OUTPUT_CAP: usize = 64 * 1024;
const RUN_TIMEOUT: Duration = Duration::from_secs(300);

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

/// Reads the whole stream so the child never blocks on a full pipe, keeping
/// only the first `OUTPUT_CAP` bytes.
fn capture(mut stream: impl Read + Send + 'static) -> JoinHandle<Captured> {
    thread::spawn(move || {
        let mut kept = Vec::new();
        let mut truncated = false;
        let mut buf = [0u8; 8192];
        loop {
            let read = match stream.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(read) => read,
            };
            let room = OUTPUT_CAP - kept.len();
            if read > room {
                truncated = true;
            }
            kept.extend_from_slice(&buf[..read.min(room)]);
        }
        Captured {
            text: String::from_utf8_lossy(&kept).into_owned(),
            truncated,
        }
    })
}

fn joined(handle: JoinHandle<Captured>) -> Captured {
    handle.join().unwrap_or(Captured {
        text: String::new(),
        truncated: true,
    })
}

/// Kills the child's whole process group, so background children that hold
/// the output pipes open die with it.
fn wait_or_kill(child: &mut Child, limit: Duration) -> io::Result<Option<ExitStatus>> {
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(Some(status)),
            Ok(None) => {}
            Err(e) => return Err(e),
        }
        if started.elapsed() >= limit {
            let _ = killpg(Pid::from_raw(child.id() as i32), Signal::SIGKILL);
            let _ = child.wait();
            return Ok(None);
        }
        thread::sleep(Duration::from_millis(50));
    }
}

fn run_captured(
    cmd: &[String],
    dir: &Path,
    env: &HashMap<String, String>,
    limit: Duration,
) -> io::Result<RunOutcome> {
    let mut child = match Command::new(&cmd[0])
        .args(&cmd[1..])
        .current_dir(dir)
        .envs(env)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()
    {
        Ok(child) => child,
        Err(e) => return Err(e),
    };
    let stdout = child.stdout.take().map(capture);
    let stderr = child.stderr.take().map(capture);
    let status = match wait_or_kill(&mut child, limit) {
        Ok(status) => status,
        Err(e) => return Err(e),
    };
    let empty = || Captured {
        text: String::new(),
        truncated: false,
    };
    Ok(RunOutcome {
        status,
        stdout: stdout.map(joined).unwrap_or_else(empty),
        stderr: stderr.map(joined).unwrap_or_else(empty),
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
pub fn cmd_agent_run(paths: &RikuPaths, app: &str, cmd: &[String]) -> AgentResponse {
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
    let mut env = HashMap::new();
    let live_env = paths.env_root.join(&app).join("LIVE_ENV");
    if let Err(e) = parse_settings(&live_env, &mut env) {
        return AgentResponse::error("COMMAND_FAILED", &e.to_string());
    }
    let dir = paths.app_root.join(&app);
    match run_captured(cmd, &dir, &env, RUN_TIMEOUT) {
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
