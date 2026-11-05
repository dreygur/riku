//! Engine calls whose output the operator watches as they run.
//!
//! Pulls and builds take minutes and print progress. The engine inherits
//! riku's stdout and stderr, so the progress reaches the caller's log (the
//! `git push` terminal during a deploy) live instead of after the call ends.
//! The call is still stopped at its own limit (see [`super::bounded`]). The
//! engine runs in its own process group, so a Ctrl-C at the terminal reaches
//! riku but not the engine client. Because the output is not captured, a
//! failure reports the exit status only; the engine's reason is already in
//! the log.

use std::ffi::OsString;
use std::process::{Command, Stdio};
use std::time::Duration;

use anyhow::{anyhow, Result};

use super::bounded::{spawn_in_group, wait_bounded};
use super::Engine;

impl Engine {
    #[allow(clippy::question_mark)]
    pub(super) fn run_streamed(
        &self,
        args: &[OsString],
        subject: &str,
        limit: Duration,
    ) -> Result<()> {
        let command = args
            .first()
            .map(|a| a.to_string_lossy().into_owned())
            .unwrap_or_default();
        let mut child = match spawn_in_group(
            Command::new(self.program())
                .args(args)
                .stdin(Stdio::null())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit()),
        ) {
            Ok(child) => child,
            Err(e) => {
                return Err(anyhow!(
                    "cannot run '{command}' on the container engine: {e}"
                ))
            }
        };
        let status = match wait_bounded(&mut child, &command, limit) {
            Ok(status) => status,
            Err(e) => return Err(anyhow!("cannot {command} {subject}: {e}")),
        };
        if !status.success() {
            return Err(anyhow!(
                "container engine '{command}' failed for {subject} with {status}, see the output above"
            ));
        }
        Ok(())
    }
}
