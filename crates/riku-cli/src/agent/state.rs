//! Private files the agent interface keeps between runs: the rate limit
//! counters and the audit log.
//!
//! Security model: the files live under the riku root, not in a shared
//! directory such as `/tmp`, so another local user can neither plant nor
//! read them. The directory must belong to the effective user and is forced
//! to `0700`; files are `0600`. Every file is opened with `O_NOFOLLOW`, so a
//! symlink put at the path is refused instead of followed to a file the riku
//! user can write.

use std::fs::{DirBuilder, File, Metadata, OpenOptions, Permissions};
use std::io::{Error, Result};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use nix::fcntl::OFlag;

const DIR_MODE: u32 = 0o700;
const FILE_MODE: u32 = 0o600;
const MODE_BITS: u32 = 0o777;

/// What a caller does with an opened state file.
#[derive(Clone, Copy)]
pub enum Access {
    Read,
    Rewrite,
    Append,
    /// Read and write in place, keeping the content so the caller can edit
    /// it under a lock.
    Update,
}

/// The agent state directory under `riku_root`, created if missing, owned by
/// the current user and mode `0700`. A symlink in its place, or a directory
/// owned by another user, is an error.
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
    match secure_dir(&dir, nix::unistd::geteuid().as_raw()) {
        Ok(()) => Ok(dir),
        Err(e) => Err(e),
    }
}

fn check_plain_dir(dir: &Path, meta: &Metadata, uid: u32) -> Result<()> {
    if !meta.is_dir() {
        return Err(Error::other(format!(
            "{} is not a plain directory",
            dir.display()
        )));
    }
    if meta.uid() != uid {
        return Err(Error::other(format!(
            "{} is not owned by the current user",
            dir.display()
        )));
    }
    Ok(())
}

#[allow(clippy::question_mark)]
fn secure_dir(dir: &Path, uid: u32) -> Result<()> {
    let meta = match std::fs::symlink_metadata(dir) {
        Ok(meta) => meta,
        Err(e) => return Err(e),
    };
    if let Err(e) = check_plain_dir(dir, &meta, uid) {
        return Err(e);
    }
    if meta.permissions().mode() & MODE_BITS == DIR_MODE {
        return Ok(());
    }
    std::fs::set_permissions(dir, Permissions::from_mode(DIR_MODE))
}

/// Opens `path` for `access` without following a symlink, creating it `0600`
/// when the access writes.
pub fn open_private(path: &Path, access: Access) -> Result<File> {
    let mut options = OpenOptions::new();
    match access {
        Access::Read => options.read(true),
        Access::Rewrite => options.write(true).create(true).truncate(true),
        Access::Append => options.append(true).create(true),
        Access::Update => options.read(true).write(true).create(true),
    };
    options
        .mode(FILE_MODE)
        .custom_flags(OFlag::O_NOFOLLOW.bits())
        .open(path)
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
