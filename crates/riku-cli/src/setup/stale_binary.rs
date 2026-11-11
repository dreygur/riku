//! Removes the deploy user's old `~/.local/bin/riku`.
//!
//! Earlier versions installed riku there. It comes before `/usr/local/bin`
//! on the usual PATH, so after an upgrade it would keep running the old
//! binary. Runs as the deploy user, in its own home.
//!
//! Security: the path is inspected with `symlink_metadata` and a link is
//! removed itself, never followed. Directories and other file types are left
//! alone.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use anyhow::{anyhow, Result};

use crate::util::echo;

const STALE_BINARY: &str = ".local/bin/riku";

/// Delete `<home>/.local/bin/riku` when it is a regular file or a symlink.
pub(super) fn remove_stale_binary(home: &Path) -> Result<()> {
    let path = home.join(STALE_BINARY);
    let kind = match fs::symlink_metadata(&path) {
        Ok(meta) => meta.file_type(),
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(anyhow!("failed to inspect {}: {e}", path.display())),
    };
    if !kind.is_file() && !kind.is_symlink() {
        return Ok(());
    }
    if let Err(e) = fs::remove_file(&path) {
        return Err(anyhow!("failed to remove {}: {e}", path.display()));
    }
    echo(
        "Removed ~/.local/bin/riku, /usr/local/bin/riku replaces it",
        "green",
    );
    Ok(())
}

#[cfg(test)]
#[path = "stale_binary_tests.rs"]
mod tests;
