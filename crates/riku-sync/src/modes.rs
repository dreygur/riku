//! Directory modes of a staged tree.
//!
//! Modes are set last, once every file is in place and verified, and deepest
//! first: a directory without write or search permission would otherwise
//! block filling it or reaching what lies below it. Each directory is checked
//! to be a real directory (never a symlink) before its mode is set, and the
//! mode is read back afterwards.

use crate::wire::{EntryKind, Manifest};
use std::fs;
use std::os::unix::fs::PermissionsExt;
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
    match fs::symlink_metadata(dir) {
        Ok(meta) if meta.file_type().is_dir() => {}
        Ok(_) => return Err(anyhow::anyhow!("{name} is not a directory in staging")),
        Err(e) => return Err(anyhow::anyhow!("cannot stat {name}: {e}")),
    }
    if let Err(e) = fs::set_permissions(dir, fs::Permissions::from_mode(mode)) {
        return Err(anyhow::anyhow!("cannot set mode of {name}: {e}"));
    }
    match fs::symlink_metadata(dir) {
        Ok(meta) if meta.permissions().mode() & 0o777 == mode => Ok(()),
        Ok(meta) => Err(anyhow::anyhow!(
            "{name} has mode {:o} after setting {mode:o}",
            meta.permissions().mode() & 0o777
        )),
        Err(e) => Err(anyhow::anyhow!("cannot stat {name}: {e}")),
    }
}
