//! Viewing and collecting container logs.
//!
//! Security model: container ids are [`ContainerId`]s (hex only) and reach
//! the engine as a separate argv element, never through a shell. A `--since`
//! bound is a timestamp the caller formatted itself, not engine output.

use std::ffi::OsString;

use anyhow::{anyhow, Result};

use super::bounded::capture;
use super::{ContainerId, Engine};

/// Which part of a container's log history [`Engine::timestamped_logs`] returns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogRange {
    /// The whole history.
    All,
    /// Lines stamped at or after this RFC 3339 time. Engines include lines
    /// stamped exactly at the bound.
    Since(String),
    /// Only the newest line.
    Newest,
}

/// The output of one bounded `logs --timestamps` call. Every line starts
/// with the engine's RFC 3339 timestamp and a space.
#[derive(Debug, Default)]
pub struct TimestampedLogs {
    /// What the container wrote to stdout.
    pub stdout: String,
    /// What the container wrote to stderr.
    pub stderr: String,
}

fn args_of<const N: usize>(parts: [&str; N]) -> Vec<OsString> {
    parts.into_iter().map(OsString::from).collect()
}

fn range_args(range: &LogRange) -> Vec<OsString> {
    match range {
        LogRange::All => Vec::new(),
        LogRange::Since(at) => vec!["--since".into(), at.into()],
        LogRange::Newest => args_of(["--tail", "1"]),
    }
}

impl Engine {
    /// Stream the container's logs to stdout and stderr.
    ///
    /// Bounded by the engine build limit.
    pub fn print_logs(&self, id: &ContainerId) -> Result<()> {
        self.run_streamed(
            &args_of(["logs", id.as_str()]),
            id.as_str(),
            self.settings().build_timeout,
        )
    }

    /// Read the part of the container's logs `range` names, with timestamps,
    /// within the call limit. The engine process exits when it is done, so
    /// no client stays running between calls.
    #[allow(clippy::question_mark)]
    pub fn timestamped_logs(&self, id: &ContainerId, range: &LogRange) -> Result<TimestampedLogs> {
        let mut args = args_of(["logs", "--timestamps"]);
        args.extend(range_args(range));
        args.push(id.as_str().into());
        let out = match capture(self.program(), &args, self.settings().call_timeout) {
            Ok(out) => out,
            Err(e) => return Err(e),
        };
        if !out.status.success() {
            return Err(anyhow!(
                "container engine 'logs' failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        if out.truncated {
            return Err(anyhow!(
                "container engine 'logs' output may be incomplete: a helper held the pipe open past the limit"
            ));
        }
        Ok(TimestampedLogs {
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        })
    }
}

#[cfg(test)]
#[path = "logs_tests.rs"]
mod tests;
