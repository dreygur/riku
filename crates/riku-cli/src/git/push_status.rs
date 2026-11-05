//! Carrying a failed deploy from the post-receive hook back to `git push`.
//!
//! git ignores the exit status of post-receive, so a deploy that fails there
//! still lets the push exit 0. `riku git-receive-pack` creates an empty
//! status file and names it in [`STATUS_ENV`] for git-receive-pack, whose
//! hooks inherit the environment. `riku git-hook` writes its error into that
//! file, and once git-receive-pack returns the push exits non-zero when the
//! file is not empty. The refs are already updated by then: the push fails,
//! but the commits are on the server.
//!
//! Security: the hook writes only to a file that already exists and never
//! creates one, and the path comes from the environment of a process that
//! already runs as the riku user.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

use crate::util::echo;

/// Names the status file of the push a hook runs for.
pub(crate) const STATUS_ENV: &str = "RIKU_PUSH_STATUS_FILE";

fn write_failure(status_file: &Path, err: &anyhow::Error) -> io::Result<()> {
    OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(status_file)
        .and_then(|mut file| write!(file, "{err:#}"))
}

/// Record `err` for the push that ran this hook. Does nothing when the hook
/// was not started through `riku git-receive-pack`.
pub(crate) fn record_deploy_failure(err: &anyhow::Error) {
    let status_file = match std::env::var_os(STATUS_ENV) {
        Some(path) => path,
        None => return,
    };
    if let Err(e) = write_failure(Path::new(&status_file), err) {
        echo(
            &format!("cannot report the failed deploy to the push, it may exit 0: {e}"),
            "yellow",
        );
    }
}

/// The deploy error the hook recorded in `status_file`, if any.
pub(crate) fn deploy_failure(status_file: &Path) -> Option<String> {
    match fs::read_to_string(status_file) {
        Ok(reason) if !reason.trim().is_empty() => Some(reason.trim().to_string()),
        Ok(_) => None,
        Err(e) => Some(format!("cannot read the deploy result: {e}")),
    }
}

#[cfg(test)]
#[path = "push_status_tests.rs"]
mod tests;
