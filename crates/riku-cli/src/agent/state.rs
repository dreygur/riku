//! Private files the agent interface keeps between runs: the rate limit
//! counters and the audit log.
//!
//! Security model: the files live under the riku root, not in a shared
//! directory such as `/tmp`, so another local user can neither plant nor
//! read them. The directory is created `0700` and files `0600`. Every file
//! is opened with `O_NOFOLLOW`, so a symlink put at the path is refused
//! instead of followed to a file the riku user can write.

use std::fs::{DirBuilder, File, OpenOptions};
use std::io::{Error, Result};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use nix::fcntl::OFlag;

const DIR_MODE: u32 = 0o700;
const FILE_MODE: u32 = 0o600;

/// What a caller does with an opened state file.
#[derive(Clone, Copy)]
pub enum Access {
    Read,
    Rewrite,
    Append,
}

/// The agent state directory under `riku_root`, created if missing. A
/// symlink in its place is an error.
#[allow(clippy::question_mark)]
pub fn agent_dir(riku_root: &Path) -> Result<PathBuf> {
    let dir = riku_root.join("state").join("agent");
    if let Err(e) = DirBuilder::new()
        .recursive(true)
        .mode(DIR_MODE)
        .create(&dir)
    {
        return Err(e);
    }
    match std::fs::symlink_metadata(&dir) {
        Ok(meta) if meta.is_dir() => Ok(dir),
        Ok(_) => Err(Error::other(format!(
            "{} is not a plain directory",
            dir.display()
        ))),
        Err(e) => Err(e),
    }
}

/// Opens `path` for `access` without following a symlink, creating it `0600`
/// when the access writes.
pub fn open_private(path: &Path, access: Access) -> Result<File> {
    let mut options = OpenOptions::new();
    match access {
        Access::Read => options.read(true),
        Access::Rewrite => options.write(true).create(true).truncate(true),
        Access::Append => options.append(true).create(true),
    };
    options
        .mode(FILE_MODE)
        .custom_flags(OFlag::O_NOFOLLOW.bits())
        .open(path)
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
