//! Atomic file writes: a hidden temp file in the destination's directory,
//! `fsync`, then `rename` over the destination.
//!
//! Security model: the destination directory may be writable by someone who
//! wants the write to land elsewhere (a link planted at the temp name). The
//! temp file is therefore created exclusively (`O_CREAT | O_EXCL`) with
//! `O_NOFOLLOW`, so an existing file or symlink at that name is never opened,
//! and the name is skipped for the next one. Each call takes a fresh name
//! (pid plus a process wide counter), so threads in one process never share a
//! temp file. Only a temp file this call created is ever removed.

// Project rule 16 forbids `?`, and this lint asks for exactly that rewrite.
#![allow(clippy::question_mark)]

use anyhow::{anyhow, Result};
use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::PRIVATE_FILE_MODE;

/// Write `contents` to `path` atomically.
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
    write_atomic_with(path, contents, Access::Shared, next_temp_seq)
}

/// [`write_atomic`] for a file only its owner may read: ENV files, worker
/// configs (which carry the app's environment) and the like. The temp file
/// is created `0600`, so the contents are never readable by others, not even
/// between create and rename.
pub fn write_private(path: &Path, contents: &[u8]) -> Result<()> {
    write_atomic_with(path, contents, Access::Private, next_temp_seq)
}

const SHARED_FILE_MODE: u32 = 0o666;

/// Temp names tried before giving up, each one already taken by a leftover
/// file or a planted link.
const TEMP_ATTEMPTS: u32 = 16;

static TEMP_SEQ: AtomicU64 = AtomicU64::new(0);

/// Who may read a file [`write_atomic_with`] writes.
enum Access {
    /// `0666` less the umask.
    Shared,
    /// `0600` whatever the umask.
    Private,
}

fn next_temp_seq() -> u64 {
    TEMP_SEQ.fetch_add(1, Ordering::Relaxed)
}

fn parent_dir(path: &Path) -> &Path {
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

fn temp_path(path: &Path, seq: u64) -> PathBuf {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy())
        .unwrap_or_default();
    parent_dir(path).join(format!(".{name}.tmp-{}-{seq}", std::process::id()))
}

fn write_atomic_with(
    path: &Path,
    contents: &[u8],
    access: Access,
    next_seq: impl FnMut() -> u64,
) -> Result<()> {
    let dir = parent_dir(path);
    if let Err(e) = fs::create_dir_all(dir) {
        return Err(anyhow!("cannot create {}: {e}", dir.display()));
    }

    let (tmp_path, tmp_file) = match create_temp(path, &access, next_seq) {
        Ok(created) => created,
        Err(e) => return Err(e),
    };
    if let Err(e) = write_synced(tmp_file, &tmp_path, contents, &access) {
        let _ = fs::remove_file(&tmp_path);
        return Err(e);
    }
    if let Err(e) = fs::rename(&tmp_path, path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(anyhow!(
            "renaming {} to {}: {e}",
            tmp_path.display(),
            path.display()
        ));
    }
    Ok(())
}

fn create_temp(
    path: &Path,
    access: &Access,
    mut next_seq: impl FnMut() -> u64,
) -> Result<(PathBuf, File)> {
    let create_mode = match access {
        Access::Shared => SHARED_FILE_MODE,
        Access::Private => PRIVATE_FILE_MODE,
    };
    for _ in 0..TEMP_ATTEMPTS {
        let tmp_path = temp_path(path, next_seq());
        let opened = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(create_mode)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&tmp_path);
        match opened {
            Ok(file) => return Ok((tmp_path, file)),
            Err(e) if e.kind() == ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(anyhow!("creating temp file {}: {e}", tmp_path.display())),
        }
    }
    Err(anyhow!(
        "no free temp file name beside {} after {TEMP_ATTEMPTS} tries",
        path.display()
    ))
}

fn write_synced(
    mut tmp_file: File,
    tmp_path: &Path,
    contents: &[u8],
    access: &Access,
) -> Result<()> {
    // The umask can take bits off the create mode; a private file is 0600 exactly.
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

#[cfg(test)]
#[path = "atomic_tests.rs"]
mod tests;
