//! Crash recovery and the final directory swap.
//!
//! Staging lives beside the live directory as `<live>.sync-new` and the old
//! tree is parked as `<live>.sync-old` during the swap, so both renames stay on
//! one filesystem. If the process dies between the two renames, live is missing
//! and `.sync-old` holds the previous tree; [`recover`] puts it back before the
//! next apply starts.

use crate::lock::SyncLock;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub(crate) const STAGING_SUFFIX: &str = ".sync-new";
const PARKED_SUFFIX: &str = ".sync-old";

/// `<live><suffix>` in the same parent directory.
pub(crate) fn sibling(live: &Path, suffix: &str) -> anyhow::Result<PathBuf> {
    let name = match live.file_name() {
        Some(name) => name,
        None => return Err(anyhow::anyhow!("{} has no directory name", live.display())),
    };
    let mut full: OsString = name.to_os_string();
    full.push(suffix);
    Ok(live.with_file_name(full))
}

fn exists(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}

/// Removes a file, link or directory tree; a missing path is fine. A symlink is
/// removed as a link, never followed.
pub(crate) fn remove_tree(path: &Path) -> io::Result<()> {
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    if meta.file_type().is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

/// Restores a parked tree when live is missing, then removes any leftover
/// staging and parked directories from an interrupted apply. Needs the sync
/// lock, since it renames and deletes beside `live`.
pub(crate) fn recover(live: &Path, _held: &SyncLock) -> anyhow::Result<()> {
    let parked = match sibling(live, PARKED_SUFFIX) {
        Ok(parked) => parked,
        Err(e) => return Err(e),
    };
    let staging = match sibling(live, STAGING_SUFFIX) {
        Ok(staging) => staging,
        Err(e) => return Err(e),
    };
    if !exists(live) && exists(&parked) {
        if let Err(e) = fs::rename(&parked, live) {
            return Err(anyhow::anyhow!(
                "cannot restore {} after an interrupted sync: {e}",
                parked.display()
            ));
        }
    }
    for leftover in [&staging, &parked] {
        if let Err(e) = remove_tree(leftover) {
            return Err(anyhow::anyhow!(
                "cannot remove leftover {}: {e}",
                leftover.display()
            ));
        }
    }
    Ok(())
}

/// Replaces `live` with `staging`. If live cannot be replaced, the old tree is
/// put back.
pub(crate) fn swap_in(live: &Path, staging: &Path) -> anyhow::Result<()> {
    let parked = match sibling(live, PARKED_SUFFIX) {
        Ok(parked) => parked,
        Err(e) => return Err(e),
    };
    let had_live = exists(live);
    if had_live {
        if let Err(e) = fs::rename(live, &parked) {
            return Err(anyhow::anyhow!("cannot move {} aside: {e}", live.display()));
        }
    }
    if let Err(e) = fs::rename(staging, live) {
        if had_live {
            if let Err(restore) = fs::rename(&parked, live) {
                return Err(anyhow::anyhow!(
                    "cannot install new tree ({e}) and cannot restore the old one from {}: {restore}",
                    parked.display()
                ));
            }
        }
        return Err(anyhow::anyhow!(
            "cannot install new tree at {}: {e}",
            live.display()
        ));
    }
    if had_live {
        if let Err(e) = remove_tree(&parked) {
            tracing::warn!(
                "sync swapped in, but cannot remove {}: {e}; the next sync removes it",
                parked.display()
            );
        }
    }
    Ok(())
}
