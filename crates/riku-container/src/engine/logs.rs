//! Viewing and following container logs.
//!
//! Security model: container ids are [`ContainerId`]s (hex only) and reach
//! the engine as a separate argv element, never through a shell.

use std::ffi::OsString;
use std::process::Command;

use anyhow::Result;

use super::{ContainerId, Engine};

fn args_of<const N: usize>(parts: [&str; N]) -> Vec<OsString> {
    parts.into_iter().map(OsString::from).collect()
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

    /// A `logs` command for the caller to spawn and stream.
    ///
    /// Streams by design and is deliberately not time bounded. When given,
    /// `tail` limits the history to that many lines.
    pub fn logs_command(&self, id: &ContainerId, tail: Option<u32>) -> Command {
        let mut command = Command::new(self.program());
        command.args(["logs", "-f"]);
        if let Some(lines) = tail {
            command.args(["--tail", &lines.to_string()]);
        }
        command.arg(id.as_str());
        command
    }
}

#[cfg(test)]
#[path = "logs_tests.rs"]
mod tests;
