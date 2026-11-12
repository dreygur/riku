//! File system utilities: recursive copy, directory counting, atomic writes
//! and symlink swaps.

use anyhow::{anyhow, Result};
use std::fs::{self, DirBuilder};
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::Path;

mod atomic;
mod symlink;

pub use atomic::{write_atomic, write_private};
pub use symlink::replace_symlink;

/// Create `dir` and any missing parents `0700`, and set `dir` itself to
/// `0700` when it already existed with a looser mode. For directories that
/// hold secrets, such as an app's env directory.
pub fn create_private_dir(dir: &Path) -> Result<()> {
    if let Err(e) = DirBuilder::new()
        .recursive(true)
        .mode(PRIVATE_DIR_MODE)
        .create(dir)
    {
        return Err(anyhow!("cannot create {}: {e}", dir.display()));
    }
    match fs::set_permissions(dir, fs::Permissions::from_mode(PRIVATE_DIR_MODE)) {
        Ok(()) => Ok(()),
        Err(e) => Err(anyhow!("cannot chmod {}: {e}", dir.display())),
    }
}

/// Set an existing file to `0600`, for a secrets file written by hand with
/// a looser mode.
pub fn restrict_to_owner(path: &Path) -> Result<()> {
    match fs::set_permissions(path, fs::Permissions::from_mode(PRIVATE_FILE_MODE)) {
        Ok(()) => Ok(()),
        Err(e) => Err(anyhow!("cannot chmod {}: {e}", path.display())),
    }
}

/// Whether a file exists, as far as the current user can tell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Presence {
    Present,
    Missing,
    /// A directory on the way cannot be searched by this user, as with the
    /// root-owned `/etc/riku/caddy` seen by the deploy user.
    Unknown,
}

/// Like [`Path::exists`], but a permission error is [`Presence::Unknown`]
/// instead of a file that is not there.
pub fn presence(path: &Path) -> Presence {
    match fs::metadata(path) {
        Ok(_) => Presence::Present,
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => Presence::Unknown,
        Err(_) => Presence::Missing,
    }
}

const PRIVATE_FILE_MODE: u32 = 0o600;
const PRIVATE_DIR_MODE: u32 = 0o700;

/// Recursively copy `source` into `dest`, skipping `.git` and `node_modules`.
pub fn copy_dir_recursive(source: &Path, dest: &Path) -> Result<()> {
    fs::create_dir_all(dest)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let src = entry.path();
        let dst = dest.join(entry.file_name());
        if src.is_dir() {
            if should_skip_dir(&src) {
                continue;
            }
            copy_dir_recursive(&src, &dst)?;
        } else {
            fs::copy(&src, &dst)?;
        }
    }
    Ok(())
}

/// Count all files (not directories) under `dir` recursively.
pub fn count_files(dir: &Path) -> Result<usize> {
    let mut count = 0;
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                count += count_files(&path)?;
            } else {
                count += 1;
            }
        }
    }
    Ok(count)
}

/// Returns true for directories that should never be copied into the app dir.
fn should_skip_dir(path: &Path) -> bool {
    path.file_name()
        .map(|n| n == ".git" || n == "node_modules" || n == ".gitignore")
        .unwrap_or(false)
}

#[cfg(test)]
#[path = "fs_tests.rs"]
mod tests;
