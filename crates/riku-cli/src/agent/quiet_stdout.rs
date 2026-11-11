//! Keeps stdout for the agent's JSON response.
//!
//! Agent commands reuse the human commands (deploy, restart, destroy), which
//! print progress to stdout and spawn git, builds and plugins that do the
//! same. Pointing file descriptor 1 at stderr for the duration covers our own
//! prints and every child process, which a print-level switch could not.

use std::io::{self, Write};

use nix::unistd::{dup, dup2_stdout};

/// Runs `work` with file descriptor 1 pointing at stderr, then restores it.
pub fn with_stdout_on_stderr<T>(work: impl FnOnce() -> T) -> anyhow::Result<T> {
    let saved = match dup(io::stdout()) {
        Ok(fd) => fd,
        Err(e) => return Err(anyhow::anyhow!("failed to save stdout: {e}")),
    };
    let _ = io::stdout().flush();
    if let Err(e) = dup2_stdout(io::stderr()) {
        return Err(anyhow::anyhow!("failed to send stdout to stderr: {e}"));
    }

    let value = work();

    let _ = io::stdout().flush();
    match dup2_stdout(&saved) {
        Ok(()) => Ok(value),
        Err(e) => Err(anyhow::anyhow!("failed to restore stdout: {e}")),
    }
}
