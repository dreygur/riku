//! Engine calls that capture or discard stdout and stderr.
//!
//! Output capture is time bounded. Calls that parse the captured stdout or
//! stderr are strict and fail when a helper kept a pipe open past the
//! deadline; calls that do not inspect stdout log a warning and succeed.

use std::ffi::OsString;
use std::time::Duration;

use anyhow::{anyhow, Result};

use super::bounded::{capture, Captured};
use super::Engine;

#[allow(clippy::question_mark)]
fn run_capture(engine: &Engine, args: &[OsString], limit: Duration) -> Result<(Captured, String)> {
    let command = match args.first() {
        Some(a) => a.to_string_lossy().into_owned(),
        None => String::new(),
    };
    let out = match capture(engine.program(), engine.env(), args, limit) {
        Ok(out) => out,
        Err(e) => return Err(e),
    };
    if !out.status.success() {
        return Err(anyhow!(
            "container engine '{command}' failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok((out, command))
}

impl Engine {
    /// Run the engine with `args`, ignoring stdout.
    ///
    /// Warns when a helper held the pipe open past the limit, but succeeds
    /// if the engine process exited cleanly.
    #[allow(clippy::question_mark)]
    pub(crate) fn run_within(&self, args: &[OsString], limit: Duration) -> Result<()> {
        let (out, command) = match run_capture(self, args, limit) {
            Ok(pair) => pair,
            Err(e) => return Err(e),
        };
        if out.truncated {
            tracing::warn!(
                command = %command,
                "container engine output may be incomplete: a helper held the pipe open past the limit"
            );
        }
        Ok(())
    }

    /// Run the engine with `args` within `call_timeout`, ignoring stdout.
    pub(crate) fn run_quiet(&self, args: &[OsString]) -> Result<()> {
        self.run_within(args, self.settings().call_timeout)
    }

    /// Run the engine with `args`, returning stdout.
    ///
    /// Fails when the engine call fails or when the output was truncated by
    /// a lingering helper holding a pipe open.
    #[allow(clippy::question_mark)]
    pub(crate) fn output_within(&self, args: &[OsString], limit: Duration) -> Result<String> {
        let (out, command) = match run_capture(self, args, limit) {
            Ok(pair) => pair,
            Err(e) => return Err(e),
        };
        if out.truncated {
            return Err(anyhow!(
                "container engine '{command}' output may be incomplete: a helper held the pipe open past the limit"
            ));
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }
}

#[cfg(test)]
#[path = "output_tests.rs"]
mod tests;
