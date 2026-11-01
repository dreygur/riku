//! Engine calls that cannot hang the caller.
//!
//! A wedged engine daemon makes `docker inspect` block forever. Every engine
//! call goes through [`capture`] or [`wait_bounded`]. The engine client runs
//! in its own process group, so on expiry the whole group gets SIGTERM, then
//! SIGKILL after [`TERM_GRACE`], and the client is reaped. Helpers the client
//! started (a credential helper, a buildkit child) go with it. The limits
//! come from [`riku_config::settings::EngineSettings`].

use std::ffi::OsString;
use std::io::{self, Read};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread::{sleep, spawn, JoinHandle};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use nix::sys::signal::{killpg, Signal};
use nix::unistd::Pid;
use riku_util::spawn::spawn_retrying_etxtbsy;

use super::identity::Engine;

const POLL_INTERVAL: Duration = Duration::from_millis(10);
const TERM_GRACE: Duration = Duration::from_secs(2);

/// Spawn `command` as the leader of a new process group.
pub(super) fn spawn_in_group(command: &mut Command) -> io::Result<Child> {
    command.process_group(0);
    spawn_retrying_etxtbsy(command)
}

fn stop_group(child: &mut Child) {
    let pgid = Pid::from_raw(child.id() as i32);
    let _ = killpg(pgid, Signal::SIGTERM);
    let deadline = Instant::now() + TERM_GRACE;
    while Instant::now() < deadline {
        let _ = child.try_wait();
        if killpg(pgid, None).is_err() {
            break;
        }
        sleep(POLL_INTERVAL);
    }
    let _ = killpg(pgid, Signal::SIGKILL);
    let _ = child.wait();
}

fn wait_until(
    child: &mut Child,
    command: &str,
    deadline: Instant,
    limit: Duration,
) -> Result<ExitStatus> {
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) => {}
            Err(e) => return Err(anyhow!("cannot wait for engine '{command}': {e}")),
        }
        if Instant::now() >= deadline {
            stop_group(child);
            return Err(anyhow!(
                "container engine '{command}' did not finish within {limit:?}"
            ));
        }
        sleep(POLL_INTERVAL);
    }
}

/// Wait for `child`, stopping its whole process group when `limit` passes
/// first. `child` must have been started with [`spawn_in_group`].
pub(super) fn wait_bounded(
    child: &mut Child,
    command: &str,
    limit: Duration,
) -> Result<ExitStatus> {
    wait_until(child, command, Instant::now() + limit, limit)
}

pub(super) fn drain(mut pipe: impl Read + Send + 'static) -> JoinHandle<Vec<u8>> {
    spawn(move || {
        let mut bytes = Vec::new();
        let _ = pipe.read_to_end(&mut bytes);
        bytes
    })
}

fn joined(handle: Option<JoinHandle<Vec<u8>>>) -> Vec<u8> {
    handle
        .map(|h| h.join().unwrap_or_default())
        .unwrap_or_default()
}

fn is_finished(handle: &Option<JoinHandle<Vec<u8>>>) -> bool {
    match handle {
        Some(h) => h.is_finished(),
        None => true,
    }
}

/// Exit status and captured output of a finished engine call.
pub(super) struct Captured {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

/// Run `program` with `args` and capture its output, within `limit`.
///
/// When it is still running after `limit` the process group is stopped and
/// the error names the subcommand and the limit.
#[allow(clippy::question_mark)]
pub(super) fn capture(program: &Path, args: &[OsString], limit: Duration) -> Result<Captured> {
    let command = args
        .first()
        .map(|a| a.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut child = match spawn_in_group(
        Command::new(program)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped()),
    ) {
        Ok(child) => child,
        Err(e) => {
            return Err(anyhow!(
                "cannot run '{command}' on the container engine: {e}"
            ))
        }
    };
    let pgid = Pid::from_raw(child.id() as i32);
    let stdout = child.stdout.take().map(drain);
    let stderr = child.stderr.take().map(drain);
    let deadline = Instant::now() + limit;
    let status = match wait_until(&mut child, &command, deadline, limit) {
        Ok(status) => status,
        Err(e) => return Err(e),
    };
    while Instant::now() < deadline {
        if is_finished(&stdout) && is_finished(&stderr) {
            break;
        }
        sleep(POLL_INTERVAL);
    }
    if !is_finished(&stdout) || !is_finished(&stderr) {
        let _ = killpg(pgid, Signal::SIGKILL);
    }
    Ok(Captured {
        status,
        stdout: joined(stdout),
        stderr: joined(stderr),
    })
}

impl Engine {
    /// Run the engine with `args`, returning stdout.
    ///
    /// A call still running after `limit` is stopped (see [`capture`]). A non
    /// zero exit is an error carrying the subcommand and trimmed stderr.
    #[allow(clippy::question_mark)]
    pub(super) fn output_within(&self, args: &[OsString], limit: Duration) -> Result<String> {
        let command = args
            .first()
            .map(|a| a.to_string_lossy().into_owned())
            .unwrap_or_default();
        let out = match capture(self.program(), args, limit) {
            Ok(out) => out,
            Err(e) => return Err(e),
        };
        if !out.status.success() {
            return Err(anyhow!(
                "container engine '{command}' failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }
}

#[cfg(test)]
#[path = "bounded_tests.rs"]
mod tests;
