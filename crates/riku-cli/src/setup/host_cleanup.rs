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
//! are taken from the nginx directories, and they are moved into a root only
//! `/etc/riku/removed/<time>` directory rather than deleted. The Caddy
//! directory sweep deletes.

use std::ffi::OsStr;
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Result};
use riku_util::write_dir::check_write_dir;

use crate::util::echo;

const REMOVED_ROOT: &str = "/etc/riku/removed";
/// The Caddy site root's own certificate dir; riku-route `caddy_certs.rs`
/// `CERT_DIR` names the same one.
const CERTS_DIR: &str = "certs";

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

/// Where untrusted nginx entries are moved: `/etc/riku/removed/<unix time>`.
pub(super) fn removed_dir_now() -> PathBuf {
    let seconds = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(since) => since.as_secs(),
        Err(_) => 0,
    };
    Path::new(REMOVED_ROOT).join(seconds.to_string())
}

/// Create the directory above `removed`'s parent (`/etc/riku`) and refuse it
/// unless only `root_uid` can write it, so no one else can swap the
/// `removed` directory for a link. Call before [`clear_deploy_owned_router_files`].
pub(super) fn check_removed_base(removed: &Path, root_uid: u32) -> Result<()> {
    let Some(base) = removed.parent().and_then(Path::parent) else {
        return Err(anyhow!("{} has no parent directories", removed.display()));
    };
    if let Err(e) = fs::create_dir_all(base) {
        return Err(anyhow!("failed to create {}: {e}", base.display()));
    }
    match check_write_dir(base, root_uid) {
        Ok(()) => Ok(()),
        Err(e) => Err(anyhow!("{e}")),
    }
}

/// Create `removed` (`/etc/riku/removed/<time>`) as a `0700` directory under a
/// `0700` parent.
fn create_removed_dir(removed: &Path) -> Result<()> {
    let Some(parent) = removed.parent() else {
        return Err(anyhow!("{} has no parent directory", removed.display()));
    };
    for dir in [parent, removed] {
        match fs::create_dir(dir) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(anyhow!("failed to create {}: {e}", dir.display())),
        }
        if let Err(e) = fs::set_permissions(dir, fs::Permissions::from_mode(0o700)) {
            return Err(anyhow!("failed to chmod {}: {e}", dir.display()));
        }
    }
    Ok(())
}

/// Move `path` to `<removed>/<dir name>/<file name>`. A rename moves a
/// symlink as a link and never follows it.
fn move_aside(path: &Path, removed: &Path) -> Result<PathBuf> {
    let (Some(dir_name), Some(file_name)) =
        (path.parent().and_then(Path::file_name), path.file_name())
    else {
        return Err(anyhow!("{} has no directory or file name", path.display()));
    };
    if let Err(e) = create_removed_dir(removed) {
        return Err(anyhow!("cannot prepare {}: {e}", removed.display()));
    }
    let dest_dir = removed.join(dir_name);
    if let Err(e) = fs::create_dir_all(&dest_dir) {
        return Err(anyhow!("failed to create {}: {e}", dest_dir.display()));
    }
    let dest = dest_dir.join(file_name);
    match fs::rename(path, &dest) {
        Ok(()) => Ok(dest),
        Err(e) => Err(anyhow!(
            "failed to move {} to {}: {e}",
            path.display(),
            dest.display()
        )),
    }
}

/// Move out of each of `dirs`, into `removed`, every file or link that
/// [`is_trusted_entry`] rejects. Nothing is deleted, so an entry that was in
/// fact wanted can be put back. Subdirectories are left in place and
/// reported. Returns the original paths of what was moved.
pub(super) fn clear_deploy_owned_router_files(
    dirs: &[&Path],
    root_uid: u32,
    removed: &Path,
) -> Result<Vec<PathBuf>> {
    let mut moved = Vec::new();
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
            let dest = match move_aside(&path, removed) {
                Ok(dest) => dest,
                Err(e) => return Err(anyhow!("{} not cleared: {e}", path.display())),
            };
            echo(
                &format!(
                    "      moved {} to {}: root nginx must not load it",
                    path.display(),
                    dest.display()
                ),
                "yellow",
            );
            moved.push(path);
        }
    }
    Ok(moved)
}

/// Clear `certs/` the way [`clear_foreign_entries`] clears the site root,
/// one level down: each trusted `certs/<app>` dir keeps its trusted files.
#[allow(clippy::question_mark)]
fn clear_cert_dirs(certs: &Path, root_uid: u32) -> Result<Vec<PathBuf>> {
    let found = match entries(certs) {
        Ok(found) => found,
        Err(e) => return Err(anyhow!("{e}")),
    };
    let mut removed = Vec::new();
    for path in found {
        let trusted = match fs::symlink_metadata(&path) {
            Ok(meta) => is_trusted_dir(&meta, root_uid),
            Err(e) => return Err(anyhow!("failed to inspect {}: {e}", path.display())),
        };
        let cleared = if trusted {
            clear_foreign_entries(&path, root_uid)
        } else {
            remove_entry(&path).map(|()| vec![path.clone()])
        };
        match cleared {
            Ok(paths) => removed.extend(paths),
            Err(e) => return Err(e),
        }
    }
    Ok(removed)
}

fn remove_entry(path: &Path) -> Result<()> {
    let is_dir = fs::symlink_metadata(path)
        .map(|meta| meta.is_dir())
        .unwrap_or(false);
    let gone = if is_dir {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    };
    if let Err(e) = gone {
        return Err(anyhow!("failed to remove {}: {e}", path.display()));
    }
    echo(
        &format!("      removed {}: not a root owned file", path.display()),
        "yellow",
    );
    Ok(())
}

/// Delete everything in `dir` that is not a regular file owned by
/// `root_uid`, symlinks and subdirectories included, except a trusted
/// `certs` dir: the router helper keeps apps' own certificates there
/// (riku-route `caddy_certs.rs`), and it is cleared one level down instead.
/// Returns what was removed.
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
        if path.file_name() == Some(OsStr::new(CERTS_DIR)) && is_trusted_dir(&meta, root_uid) {
            match clear_cert_dirs(&path, root_uid) {
                Ok(paths) => removed.extend(paths),
                Err(e) => return Err(e),
            }
            continue;
        }
        if let Err(e) = remove_entry(&path) {
            return Err(e);
        }
        removed.push(path);
    }
    Ok(removed)
}

#[cfg(test)]
#[path = "host_cleanup_tests.rs"]
mod tests;
