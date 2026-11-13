//! `riku doctor` check for the root directories the router helper writes
//! into: `/etc/riku/nginx`, `/etc/riku/certs`, `/etc/riku/caddy` and its
//! `certs`.
//!
//! The router loads what is in them, so each must be a real directory owned
//! by root that no group or other user can write; the helper refuses to
//! write or reload otherwise. Read-only: only `lstat` is called.

use std::io::ErrorKind;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use super::Check;

const NAME: &str = "router dirs";

fn dir_check(dir: &Path, root_uid: u32) -> Option<Check> {
    let meta = match dir.symlink_metadata() {
        Ok(meta) => meta,
        Err(e) if e.kind() == ErrorKind::NotFound => return None,
        Err(e) if e.kind() == ErrorKind::PermissionDenied => {
            return Some(Check::ok(
                NAME,
                format!(
                    "{}: not checked, this user cannot see it (sudo riku doctor checks it)",
                    dir.display()
                ),
            ))
        }
        Err(e) => {
            return Some(Check::fail(
                NAME,
                format!("{}: cannot inspect: {e}", dir.display()),
            ))
        }
    };
    if !meta.is_dir() {
        return Some(Check::fail(
            NAME,
            format!(
                "{} is not a directory (a symlink?); remove it and run sudo riku init",
                dir.display()
            ),
        ));
    }
    let mode = meta.mode() & 0o7777;
    let detail = format!("{}: owner uid {}, mode {mode:o}", dir.display(), meta.uid());
    if meta.uid() != root_uid || mode & 0o022 != 0 {
        return Some(Check::fail(
            NAME,
            format!("{detail}; only root may write it, run sudo riku init to fix it"),
        ));
    }
    Some(Check::ok(NAME, detail))
}

/// One line per directory of `dirs` that exists, failing any that is not a
/// real directory owned by `root_uid` and writable by it alone. A single ok
/// line when none exists yet.
pub fn router_dirs(dirs: &[PathBuf], root_uid: u32) -> Vec<Check> {
    let checks: Vec<Check> = dirs
        .iter()
        .filter_map(|dir| dir_check(dir, root_uid))
        .collect();
    if checks.is_empty() {
        return vec![Check::ok(NAME, "none created yet")];
    }
    checks
}

#[cfg(test)]
#[path = "router_dirs_check_tests.rs"]
mod tests;
