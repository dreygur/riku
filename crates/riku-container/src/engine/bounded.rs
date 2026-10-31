//! Engine calls that cannot hang the caller.
//!
//! A wedged engine daemon makes `docker inspect` block forever. Every engine
//! call goes through [`Engine::output_within`] or [`wait_bounded`], which kill
//! and reap the child once the limit passes. The limits come from
//! [`riku_config::settings::EngineSettings`].

use std::ffi::OsString;
use std::io::Read;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread::{sleep, spawn, JoinHandle};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use riku_util::spawn::spawn_retrying_etxtbsy;

use super::identity::Engine;

const POLL_INTERVAL: Duration = Duration::from_millis(10);

/// Wait for `child`, killing and reaping it when `limit` passes first.
pub(super) fn wait_bounded(
    child: &mut Child,
    command: &str,
    limit: Duration,
) -> Result<ExitStatus> {
    let deadline = Instant::now() + limit;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) => {}
            Err(e) => return Err(anyhow!("cannot wait for engine '{command}': {e}")),
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(anyhow!(
                "container engine '{command}' did not finish within {limit:?}"
            ));
        }
        sleep(POLL_INTERVAL);
    }
}

pub(super) fn drain(mut pipe: impl Read + Send + 'static) -> JoinHandle<Vec<u8>> {
    spawn(move || {
        let mut bytes = Vec::new();
        let _ = pipe.read_to_end(&mut bytes);
        bytes
    })
}

impl Engine {
    /// Run the engine with `args`, returning stdout.
    ///
    /// When the engine is still running after `limit` it is killed and reaped,
    /// and the error names the subcommand and the limit. A non zero exit is an
    /// error carrying the subcommand and trimmed stderr.
    #[allow(clippy::question_mark)]
    pub(super) fn output_within(&self, args: &[OsString], limit: Duration) -> Result<String> {
        let command = args
            .first()
            .map(|a| a.to_string_lossy().into_owned())
            .unwrap_or_default();
        let mut child = match spawn_retrying_etxtbsy(
            Command::new(self.program())
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
        let stdout = child.stdout.take().map(drain);
        let stderr = child.stderr.take().map(drain);
        let status = match wait_bounded(&mut child, &command, limit) {
            Ok(status) => status,
            Err(e) => return Err(e),
        };
        let stdout = stdout
            .map(|h| h.join().unwrap_or_default())
            .unwrap_or_default();
        let stderr = stderr
            .map(|h| h.join().unwrap_or_default())
            .unwrap_or_default();
        if !status.success() {
            return Err(anyhow!(
                "container engine '{command}' failed: {}",
                String::from_utf8_lossy(&stderr).trim()
            ));
        }
        Ok(String::from_utf8_lossy(&stdout).into_owned())
    }
}

#[cfg(test)]
#[path = "bounded_tests.rs"]
mod tests;
