//! The uWSGI socket directory: created mode `2750` with the shared socket
//! group, so every socket uWSGI binds in it inherits that group and only
//! the owner and that group can reach it.
//!
//! Security: a path that is not a real directory (a symlink planted in its
//! place) is refused before any chown or chmod, so neither can be redirected
//! to another file. Callers only use a directory whose parent no other
//! account can write (`/run` on a host, a private temp dir in tests).

use std::fs;
use std::io::ErrorKind;
use std::os::unix::fs::{chown, PermissionsExt};
use std::path::Path;

use anyhow::{anyhow, Result};
use nix::unistd::Group;

/// Owner rwx, group r-x, nothing for others, and setgid so sockets made in
/// the directory take its group rather than their creator's primary group.
pub const SOCKET_DIR_MODE: u32 = 0o2750;

/// `group`'s gid, `None` when the host has no such group (a dev machine or
/// a host root init never ran on).
pub fn group_id(group: &str) -> Option<u32> {
    match Group::from_name(group) {
        Ok(Some(entry)) => Some(entry.gid.as_raw()),
        _ => None,
    }
}

/// Create `dir` if missing, then set its mode to [`SOCKET_DIR_MODE`], its
/// group to `group` when that group exists, and its owner to `owner` when
/// given. Root init passes the deploy user's uid; anyone else passes `None`.
#[allow(clippy::question_mark)]
pub fn prepare(dir: &Path, owner: Option<u32>, group: &str) -> Result<()> {
    match fs::create_dir(dir) {
        Ok(()) => {}
        Err(e) if e.kind() == ErrorKind::AlreadyExists => {}
        Err(e) => return Err(anyhow!("cannot create {}: {e}", dir.display())),
    }
    let meta = match dir.symlink_metadata() {
        Ok(meta) => meta,
        Err(e) => return Err(anyhow!("cannot inspect {}: {e}", dir.display())),
    };
    if !meta.is_dir() {
        return Err(anyhow!(
            "refusing to use {} as the socket directory: not a directory (a symlink?)",
            dir.display()
        ));
    }
    let gid = group_id(group);
    if owner.is_some() || gid.is_some() {
        if let Err(e) = chown(dir, owner, gid) {
            return Err(anyhow!("cannot chown {}: {e}", dir.display()));
        }
    }
    match fs::set_permissions(dir, fs::Permissions::from_mode(SOCKET_DIR_MODE)) {
        Ok(()) => Ok(()),
        Err(e) => Err(anyhow!("cannot chmod {}: {e}", dir.display())),
    }
}

/// [`prepare`] `dir` unless it is already a directory. Deploys and the
/// supervisor call it: after a reboot on a host without systemd-tmpfiles the
/// directory is gone, and only an account that can write its parent can
/// make it again.
pub fn ensure_exists(dir: &Path, group: &str) -> Result<()> {
    if dir.is_dir() {
        return Ok(());
    }
    match prepare(dir, None, group) {
        Ok(()) => Ok(()),
        Err(e) => Err(anyhow!(
            "the uWSGI socket directory {} is missing and cannot be created ({e}); \
             run `sudo riku init` to create it",
            dir.display()
        )),
    }
}

#[cfg(test)]
#[path = "socket_dir_tests.rs"]
mod tests;
