//! Removes files an older riku left where root's nginx or Caddy would load
//! them, but that the deploy user can still write.
//!
//! Security: older versions put symlinks and riku named site files into the
//! system nginx directories and kept Caddy's sites inside the deploy home.
//! Root's service loading any file the deploy user controls is root code
//! execution, so setup deletes them. Ownership decides, not names or paths:
//! in the nginx directories a file must be root owned and writable by no one
//! else, and a link is kept only when its written target is such a file and
//! every directory on the way is a real, root owned directory nobody else
//! can write. Nothing is resolved by following links. Only files and links
//! are removed from the nginx directories.

use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};

use crate::util::echo;

fn entries(dir: &Path) -> Result<Vec<PathBuf>> {
    let listing = match fs::read_dir(dir) {
        Ok(listing) => listing,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(anyhow!("failed to list {}: {e}", dir.display())),
    };
    let mut paths = Vec::new();
    for entry in listing {
        match entry {
            Ok(entry) => paths.push(entry.path()),
            Err(e) => return Err(anyhow!("failed to list {}: {e}", dir.display())),
        }
    }
    Ok(paths)
}

/// A regular file owned by `root_uid` that no one else can write.
pub(super) fn is_trusted_file(meta: &fs::Metadata, root_uid: u32) -> bool {
    meta.is_file() && meta.uid() == root_uid && meta.mode() & 0o022 == 0
}

fn is_trusted_dir(meta: &fs::Metadata, root_uid: u32) -> bool {
    meta.is_dir() && (meta.uid() == root_uid || meta.uid() == 0) && meta.mode() & 0o022 == 0
}

/// Whether root's nginx may keep loading `path`: a [`is_trusted_file`], or a
/// link whose written target is one, reached through real directories only.
/// Every component of the target as written is checked without following
/// links, so no link anywhere on the way (which someone else could retarget)
/// is accepted.
fn is_trusted_entry(path: &Path, meta: &fs::Metadata, root_uid: u32) -> bool {
    if !meta.file_type().is_symlink() {
        return is_trusted_file(meta, root_uid);
    }
    let Ok(written) = fs::read_link(path) else {
        return false;
    };
    let target = path.parent().unwrap_or(Path::new("/")).join(written);
    let target_trusted = match fs::symlink_metadata(&target) {
        Ok(target_meta) => is_trusted_file(&target_meta, root_uid),
        Err(_) => false,
    };
    target_trusted
        && target
            .ancestors()
            .skip(1)
            .all(|dir| match fs::symlink_metadata(dir) {
                Ok(dir_meta) => is_trusted_dir(&dir_meta, root_uid),
                Err(_) => false,
            })
}

/// Delete from each of `dirs` every file or link that [`is_trusted_entry`]
/// rejects. Subdirectories are left in place and reported. Returns what was
/// removed.
pub(super) fn clear_deploy_owned_router_files(
    dirs: &[&Path],
    root_uid: u32,
) -> Result<Vec<PathBuf>> {
    let mut removed = Vec::new();
    for dir in dirs {
        let found = match entries(dir) {
            Ok(found) => found,
            Err(e) => return Err(anyhow!("{e}")),
        };
        for path in found {
            let meta = match fs::symlink_metadata(&path) {
                Ok(meta) => meta,
                Err(e) => return Err(anyhow!("failed to inspect {}: {e}", path.display())),
            };
            if meta.is_dir() {
                if meta.uid() != root_uid {
                    echo(
                        &format!(
                            "      warning: {} is a directory not owned by root; remove it by hand",
                            path.display()
                        ),
                        "yellow",
                    );
                }
                continue;
            }
            if is_trusted_entry(&path, &meta, root_uid) {
                continue;
            }
            if let Err(e) = fs::remove_file(&path) {
                return Err(anyhow!("failed to remove {}: {e}", path.display()));
            }
            echo(
                &format!(
                    "      removed {}: root nginx must not load it",
                    path.display()
                ),
                "yellow",
            );
            removed.push(path);
        }
    }
    Ok(removed)
}

/// Delete everything in `dir` that is not a regular file owned by
/// `root_uid`, symlinks and subdirectories included. Returns what was removed.
pub(super) fn clear_foreign_entries(dir: &Path, root_uid: u32) -> Result<Vec<PathBuf>> {
    let found = match entries(dir) {
        Ok(found) => found,
        Err(e) => return Err(anyhow!("{e}")),
    };
    let mut removed = Vec::new();
    for path in found {
        let meta = match fs::symlink_metadata(&path) {
            Ok(meta) => meta,
            Err(e) => return Err(anyhow!("failed to inspect {}: {e}", path.display())),
        };
        if is_trusted_file(&meta, root_uid) {
            continue;
        }
        let gone = if meta.is_dir() {
            fs::remove_dir_all(&path)
        } else {
            fs::remove_file(&path)
        };
        if let Err(e) = gone {
            return Err(anyhow!("failed to remove {}: {e}", path.display()));
        }
        echo(
            &format!("      removed {}: not a root owned file", path.display()),
            "yellow",
        );
        removed.push(path);
    }
    Ok(removed)
}

#[cfg(test)]
#[path = "host_cleanup_tests.rs"]
mod tests;
