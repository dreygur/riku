//! Removes files an older riku left where root's nginx or Caddy would load
//! them, but that the deploy user can still write.
//!
//! Security: older versions put symlinks and riku named site files into the
//! system nginx directories and kept Caddy's sites inside the deploy home.
//! Root's service loading any file the deploy user controls is root code
//! execution, so setup deletes them. Ownership decides, not names or paths:
//! in the nginx directories a file must be root owned, and a link must end
//! at a root owned file that neither group nor others can write. Entries
//! are inspected with `symlink_metadata`, and only files and links are
//! removed from the nginx directories.

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

/// Whether every directory from `/` down to `dir` is owned by `root_uid` (or
/// by real root) and writable by no one else, so nobody but root can rename
/// or replace what is inside.
fn is_trusted_dir_chain(dir: &Path, root_uid: u32) -> bool {
    dir.ancestors()
        .all(|ancestor| match fs::metadata(ancestor) {
            Ok(meta) => {
                meta.is_dir()
                    && (meta.uid() == root_uid || meta.uid() == 0)
                    && meta.mode() & 0o022 == 0
            }
            Err(_) => false,
        })
}

/// Whether root's nginx may keep loading `path`: a [`is_trusted_file`], or a
/// link that points directly at one inside a trusted directory chain. A
/// dangling link, or a link to another link, is not trusted.
fn is_trusted_entry(path: &Path, meta: &fs::Metadata, root_uid: u32) -> bool {
    if !meta.file_type().is_symlink() {
        return is_trusted_file(meta, root_uid);
    }
    let Ok(written) = fs::read_link(path) else {
        return false;
    };
    let direct = path.parent().unwrap_or(Path::new("/")).join(written);
    match fs::symlink_metadata(&direct) {
        Ok(direct_meta) if !direct_meta.file_type().is_symlink() => {}
        _ => return false,
    }
    let Ok(resolved) = fs::canonicalize(&direct) else {
        return false;
    };
    let Some(parent) = resolved.parent() else {
        return false;
    };
    match fs::metadata(&resolved) {
        Ok(target) => is_trusted_file(&target, root_uid) && is_trusted_dir_chain(parent, root_uid),
        Err(_) => false,
    }
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
        if meta.is_file() && meta.uid() == root_uid {
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
