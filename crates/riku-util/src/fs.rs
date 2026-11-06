//! File system utilities: recursive copy, directory counting, atomic writes
//! and symlink swaps.

use anyhow::{anyhow, Result};
use std::fs::{self, DirBuilder, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::Path;

/// Write `contents` to `path` atomically: write to a `.tmp-<pid>` sibling in
/// the same directory, `fsync` it, then `rename` over the destination.
///
/// A direct `fs::write` leaves a truncated/corrupt file if the process dies
/// (crash, OOM-kill) mid-write; a concurrent reader (e.g. the supervisor's
/// `notify::Watcher` picking up worker TOML / nginx conf changes) can then
/// observe that half-written state. `rename(2)` within the same filesystem
/// is atomic, so readers only ever see the old complete file or the new
/// complete file, never a partial one.
///
/// The file gets the usual `0666` less the umask. Use [`write_private`] for
/// anything holding secrets.
pub fn write_atomic(path: &Path, contents: &[u8]) -> Result<()> {
    write_atomic_as(path, contents, Access::Shared)
}

/// [`write_atomic`] for a file only its owner may read: ENV files, worker
/// configs (which carry the app's environment) and the like. The temp file
/// is created `0600`, so the contents are never readable by others, not even
/// between create and rename.
pub fn write_private(path: &Path, contents: &[u8]) -> Result<()> {
    write_atomic_as(path, contents, Access::Private)
}

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

/// Set an existing file to `0600`, for a secrets file written by hand or by
/// an older riku with a looser mode.
pub fn restrict_to_owner(path: &Path) -> Result<()> {
    match fs::set_permissions(path, fs::Permissions::from_mode(PRIVATE_FILE_MODE)) {
        Ok(()) => Ok(()),
        Err(e) => Err(anyhow!("cannot chmod {}: {e}", path.display())),
    }
}

const SHARED_FILE_MODE: u32 = 0o666;
const PRIVATE_FILE_MODE: u32 = 0o600;
const PRIVATE_DIR_MODE: u32 = 0o700;

/// Who may read a file [`write_atomic_as`] writes.
enum Access {
    /// `0666` less the umask.
    Shared,
    /// `0600` whatever the umask.
    Private,
}

fn write_synced(tmp_path: &Path, contents: &[u8], access: Access) -> Result<()> {
    let create_mode = match access {
        Access::Shared => SHARED_FILE_MODE,
        Access::Private => PRIVATE_FILE_MODE,
    };
    let mut tmp_file = match OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(create_mode)
        .open(tmp_path)
    {
        Ok(file) => file,
        Err(e) => return Err(anyhow!("creating temp file {}: {e}", tmp_path.display())),
    };
    // A temp file left by a crashed run keeps its old mode through `open`.
    if let Access::Private = access {
        let private = fs::Permissions::from_mode(PRIVATE_FILE_MODE);
        if let Err(e) = tmp_file.set_permissions(private) {
            return Err(anyhow!("cannot chmod {}: {e}", tmp_path.display()));
        }
    }
    if let Err(e) = tmp_file.write_all(contents) {
        return Err(anyhow!("writing {}: {e}", tmp_path.display()));
    }
    match tmp_file.sync_all() {
        Ok(()) => Ok(()),
        Err(e) => Err(anyhow!("syncing {}: {e}", tmp_path.display())),
    }
}

fn write_atomic_as(path: &Path, contents: &[u8], access: Access) -> Result<()> {
    let dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    if let Err(e) = fs::create_dir_all(dir) {
        return Err(anyhow!("cannot create {}: {e}", dir.display()));
    }

    let tmp_path = dir.join(format!(
        ".{}.tmp-{}",
        path.file_name()
            .map(|n| n.to_string_lossy())
            .unwrap_or_default(),
        std::process::id()
    ));
    if let Err(e) = write_synced(&tmp_path, contents, access) {
        let _ = fs::remove_file(&tmp_path);
        return Err(e);
    }

    match fs::rename(&tmp_path, path) {
        Ok(()) => Ok(()),
        Err(e) => Err(anyhow!(
            "renaming {} to {}: {e}",
            tmp_path.display(),
            path.display()
        )),
    }
}

/// Point the symlink `link` at `target`, replacing whatever `link` was.
///
/// The new link is made under a `.tmp-<pid>` name and renamed over `link`, so
/// `link` always resolves to the old or the new target, never to nothing. The
/// supervisor watches `workers-enabled`, and a moment without the link would
/// read as the worker being removed.
pub fn replace_symlink(target: &Path, link: &Path) -> Result<()> {
    let dir = link
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let tmp_link = dir.join(format!(
        ".{}.tmp-{}",
        link.file_name()
            .map(|n| n.to_string_lossy())
            .unwrap_or_default(),
        std::process::id()
    ));
    let _ = fs::remove_file(&tmp_link);
    if let Err(e) = std::os::unix::fs::symlink(target, &tmp_link) {
        return Err(anyhow::anyhow!("cannot link {}: {e}", tmp_link.display()));
    }
    if let Err(e) = fs::rename(&tmp_link, link) {
        let _ = fs::remove_file(&tmp_link);
        return Err(anyhow::anyhow!("cannot replace {}: {e}", link.display()));
    }
    Ok(())
}

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
