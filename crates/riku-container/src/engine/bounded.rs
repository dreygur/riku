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

/// One output stream read up to a byte cap.
#[derive(Default)]
struct Drained {
    bytes: Vec<u8>,
    capped: bool,
}

/// Read at most `cap` bytes. Past the cap the whole process group gets
/// SIGKILL, so the client neither blocks on a full pipe until the deadline
/// nor keeps the caller busy reading output it will throw away.
fn drain_capped(pipe: impl Read + Send + 'static, cap: usize, pgid: Pid) -> JoinHandle<Drained> {
    spawn(move || {
        let mut bytes = Vec::new();
        let _ = pipe
            .take((cap as u64).saturating_add(1))
            .read_to_end(&mut bytes);
        let capped = bytes.len() > cap;
        if capped {
            bytes.truncate(cap);
            let _ = killpg(pgid, Signal::SIGKILL);
        }
        Drained { bytes, capped }
    })
}

fn joined<T: Default>(handle: Option<JoinHandle<T>>) -> T {
    handle
        .map(|h| h.join().unwrap_or_default())
        .unwrap_or_default()
}

fn is_finished<T>(handle: &Option<JoinHandle<T>>) -> bool {
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
    /// True when stdout or stderr was still open when the capture deadline
    /// passed, so the output may be incomplete.
    pub truncated: bool,
    /// True when stdout reached the byte cap of [`capture_capped`] and the
    /// client was killed. `status` is then the kill, not the client's own.
    pub stdout_capped: bool,
    /// The same as `stdout_capped`, for stderr.
    pub stderr_capped: bool,
}

/// Run `program` with `args` and capture its output, within `limit`.
///
/// When it is still running after `limit` the process group is stopped and
/// the error names the subcommand and the limit. Output is marked truncated
/// when a helper held a pipe open past the limit.
pub(super) fn capture(program: &Path, args: &[OsString], limit: Duration) -> Result<Captured> {
    capture_capped(program, args, limit, usize::MAX)
}

/// [`capture`], keeping at most `cap` bytes of each stream. The first stream
/// to pass the cap stops the client; see [`Captured::stdout_capped`].
#[allow(clippy::question_mark)]
pub(super) fn capture_capped(
    program: &Path,
    args: &[OsString],
    limit: Duration,
    cap: usize,
) -> Result<Captured> {
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
    let stdout = child
        .stdout
        .take()
        .map(|pipe| drain_capped(pipe, cap, pgid));
    let stderr = child
        .stderr
        .take()
        .map(|pipe| drain_capped(pipe, cap, pgid));
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
    let truncated = !is_finished(&stdout) || !is_finished(&stderr);
    if truncated {
        let _ = killpg(pgid, Signal::SIGKILL);
    }
    let stdout = joined(stdout);
    let stderr = joined(stderr);
    Ok(Captured {
        status,
        stdout: stdout.bytes,
        stderr: stderr.bytes,
        truncated,
        stdout_capped: stdout.capped,
        stderr_capped: stderr.capped,
    })
}

#[cfg(test)]
#[path = "bounded_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "bounded_limits_tests.rs"]
mod limits_tests;
