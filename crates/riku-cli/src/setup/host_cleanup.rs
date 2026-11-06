//! Removes files an older riku left where root's nginx or Caddy would load
//! them, but that the deploy user can still write.
//!
//! Security: older versions put symlinks and riku named site files into the
//! system nginx directories and kept Caddy's sites inside the deploy home.
//! Root's service loading any file the deploy user controls is root code
//! execution, so setup deletes them. Entries are inspected with
//! `symlink_metadata`, so a link is never followed, and only files and links
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

/// Where a symlink points, resolved when the target exists and as written
/// otherwise, so a dangling link into the home is still caught.
fn link_target(link: &Path) -> Option<PathBuf> {
    if let Ok(resolved) = fs::canonicalize(link) {
        return Some(resolved);
    }
    fs::read_link(link).ok()
}

fn points_into(link: &Path, home: &Path) -> bool {
    let Some(target) = link_target(link) else {
        return false;
    };
    let real_home = fs::canonicalize(home).unwrap_or_else(|_| home.to_path_buf());
    target.starts_with(home) || target.starts_with(real_home)
}

/// Whether `path` is a symlink into `home`, or a regular file named
/// `riku*` (other than `keep`) owned by `deploy_uid`.
fn is_deploy_owned_router_file(path: &Path, home: &Path, deploy_uid: u32, keep: &str) -> bool {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return false;
    };
    if meta.file_type().is_symlink() && points_into(path, home) {
        return true;
    }
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned());
    let riku_named = name
        .as_deref()
        .is_some_and(|n| n.starts_with("riku") && n != keep);
    riku_named && !meta.is_dir() && meta.uid() == deploy_uid
}

/// Delete from each of `dirs` the entries [`is_deploy_owned_router_file`]
/// accepts. Returns what was removed.
pub(super) fn clear_deploy_owned_router_files(
    dirs: &[&Path],
    home: &Path,
    deploy_uid: u32,
    keep: &str,
) -> Result<Vec<PathBuf>> {
    let mut removed = Vec::new();
    for dir in dirs {
        let found = match entries(dir) {
            Ok(found) => found,
            Err(e) => return Err(anyhow!("{e}")),
        };
        for path in found {
            if !is_deploy_owned_router_file(&path, home, deploy_uid, keep) {
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
