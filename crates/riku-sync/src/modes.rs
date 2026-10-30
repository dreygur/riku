//! Directory modes of a staged tree.
//!
//! Modes are set last, once every file is in place and verified, and deepest
//! first: a directory without write or search permission would otherwise
//! block filling it or reaching what lies below it. Each mode is set through
//! [`chmod_dir`], which refuses a symlink in place of the directory, and is
//! read back afterwards.
//!
//! The mode is carried as sent, within `0o777`: group and other bits are not
//! masked.

use crate::wire::{EntryKind, Manifest};
use std::fs;
use std::io;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;

/// Sets every listed directory's mode, then the root's, under `root`.
pub(crate) fn apply_dir_modes(root: &Path, manifest: &Manifest) -> anyhow::Result<()> {
    for entry in manifest.entries.iter().rev() {
        if let EntryKind::Dir { mode } = &entry.kind {
            let dir = root.join(entry.path.to_path());
            if let Err(e) = set_dir_mode(&dir, entry.path.as_str(), *mode) {
                return Err(e);
            }
        }
    }
    set_dir_mode(root, "the sync root", manifest.root_mode)
}

fn set_dir_mode(dir: &Path, name: &str, mode: u32) -> anyhow::Result<()> {
    match chmod_dir(dir, |_| mode) {
        Ok(set) if set == mode => Ok(()),
        Ok(set) => Err(anyhow::anyhow!(
            "{name} has mode {set:o} after setting {mode:o}"
        )),
        Err(e) => Err(anyhow::anyhow!("cannot set mode of {name}: {e}")),
    }
}

/// Changes the mode of the directory at `dir` to `new_mode(current)` and
/// returns the permission bits it has afterwards.
///
/// The directory is opened with `O_DIRECTORY | O_NOFOLLOW` and changed with
/// `fchmod` on that descriptor, so a symlink at `dir`, even one swapped in
/// after an earlier check, is refused rather than followed. Opening needs read
/// permission on the directory for its owner.
pub(crate) fn chmod_dir(dir: &Path, new_mode: impl Fn(u32) -> u32) -> io::Result<u32> {
    let opened = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
        .open(dir);
    let handle = match opened {
        Ok(handle) => handle,
        Err(e) => return Err(e),
    };
    let current = match handle.metadata() {
        Ok(meta) => meta.permissions().mode() & 0o7777,
        Err(e) => return Err(e),
    };
    let wanted = new_mode(current);
    if wanted != current {
        if let Err(e) = handle.set_permissions(fs::Permissions::from_mode(wanted)) {
            return Err(e);
        }
    }
    match handle.metadata() {
        Ok(meta) => Ok(meta.permissions().mode() & 0o777),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
#[path = "modes_tests.rs"]
mod tests;
