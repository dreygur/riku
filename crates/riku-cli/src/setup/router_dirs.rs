//! Owner and mode of the root directories the router helper writes into,
//! set by every root init, not only the one that created them.
//!
//! Security: the router loads what is in these directories, so they must
//! stay root owned and writable by root alone. A path that is not a real
//! directory (a symlink planted in its place) is refused before any chown
//! or chmod, so neither can be redirected. Callers only pass directories
//! whose parent root owns and nobody else can write (`/etc/riku`), so the
//! entry cannot be swapped between the check and the change.

use std::fs;
use std::io::ErrorKind;
use std::os::unix::fs::{chown, PermissionsExt};
use std::path::Path;

use anyhow::{anyhow, Result};

/// Give the existing directory `dir` the owner `owner`, the group `gid` when
/// given, and exactly `mode`.
pub(super) fn secure_dir(dir: &Path, owner: u32, gid: Option<u32>, mode: u32) -> Result<()> {
    let meta = match dir.symlink_metadata() {
        Ok(meta) => meta,
        Err(e) => return Err(anyhow!("cannot inspect {}: {e}", dir.display())),
    };
    if !meta.is_dir() {
        return Err(anyhow!(
            "refusing to use {}: not a directory (a symlink?)",
            dir.display()
        ));
    }
    if let Err(e) = chown(dir, Some(owner), gid) {
        return Err(anyhow!("cannot chown {}: {e}", dir.display()));
    }
    match fs::set_permissions(dir, fs::Permissions::from_mode(mode)) {
        Ok(()) => Ok(()),
        Err(e) => Err(anyhow!("cannot chmod {}: {e}", dir.display())),
    }
}

/// [`secure_dir`] for a certificate root and each app directory in it. A
/// root that does not exist yet is left for the router helper to create.
/// An entry that is not a real directory is not touched: the helper refuses
/// it when it next writes that app's pair.
#[allow(clippy::question_mark)]
pub(super) fn secure_cert_dirs(root: &Path, owner: u32, gid: Option<u32>, mode: u32) -> Result<()> {
    match root.symlink_metadata() {
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(()),
        _ => {}
    }
    if let Err(e) = secure_dir(root, owner, gid, mode) {
        return Err(e);
    }
    let listing = match fs::read_dir(root) {
        Ok(listing) => listing,
        Err(e) => return Err(anyhow!("cannot list {}: {e}", root.display())),
    };
    listing
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .try_for_each(|entry| secure_dir(&entry.path(), owner, gid, mode))
}

#[cfg(test)]
#[path = "router_dirs_tests.rs"]
mod tests;
