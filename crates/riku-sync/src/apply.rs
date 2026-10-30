//! Verified apply of a sync bundle onto the live directory.
//!
//! Security model: see the crate docs. Nothing touches `live` until staging is
//! complete and every file in it has matched its manifest size and SHA-256. Any
//! error before the swap removes staging and leaves `live` as it was.

use crate::check::wanted_sizes;
use crate::lock::{acquire, SyncLock};
use crate::receive::receive_bundle;
use crate::staging::Staging;
use crate::swap::{recover, remove_tree, sibling, swap_in, STAGING_SUFFIX};
use crate::wire::{EntryKind, Manifest, TransferPlan};
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::Path;

/// Builds the manifest's tree in `<live>.sync-new` from `bundle` plus unchanged
/// files copied from `live`, verifies it, then swaps it in for `live`.
///
/// `bundle` is the zstd compressed tar made by `write_bundle` for `plan.need`.
/// A missing `live` is valid (first sync). Leftovers of an interrupted earlier
/// apply are cleaned up first.
///
/// Everything in `live` that the manifest does not list is removed at the swap,
/// so state the running app writes must live outside the synced directory.
///
/// An exclusive lock on `<live>.sync-lock` is held throughout; a second apply to
/// the same `live` fails at once instead of waiting.
pub fn apply(
    live: &Path,
    manifest: &Manifest,
    plan: &TransferPlan,
    bundle: impl Read,
) -> anyhow::Result<()> {
    let lock = match acquire(live) {
        Ok(lock) => lock,
        Err(e) => return Err(e),
    };
    if let Err(e) = recover(live, &lock) {
        return Err(e);
    }
    apply_locked(live, &lock, manifest, plan, bundle)
}

/// [`apply`] for a caller that already holds the lock and has already run
/// [`recover`] under it, so it could plan against the recovered tree.
///
/// A second `flock` taken in the same process conflicts with the first, so a
/// caller that locked early cannot call `apply`. `_held` is only the proof.
pub(crate) fn apply_locked(
    live: &Path,
    _held: &SyncLock,
    manifest: &Manifest,
    plan: &TransferPlan,
    bundle: impl Read,
) -> anyhow::Result<()> {
    let wanted = match wanted_sizes(manifest, plan) {
        Ok(wanted) => wanted,
        Err(e) => return Err(e),
    };
    let staging_path = match sibling(live, STAGING_SUFFIX) {
        Ok(path) => path,
        Err(e) => return Err(e),
    };
    let staging = match Staging::create(staging_path.clone()) {
        Ok(staging) => staging,
        Err(e) => return Err(e),
    };
    let built = fill_and_verify(&staging, live, manifest, &wanted, bundle);
    let outcome = match built {
        Ok(()) => swap_in(live, &staging_path),
        Err(e) => Err(e),
    };
    match outcome {
        Ok(()) => Ok(()),
        Err(e) => match remove_tree(&staging_path) {
            Ok(()) => Err(e),
            Err(cleanup) => Err(anyhow::anyhow!(
                "{e}; also cannot remove {}: {cleanup}",
                staging_path.display()
            )),
        },
    }
}

fn fill_and_verify(
    staging: &Staging,
    live: &Path,
    manifest: &Manifest,
    wanted: &HashMap<&str, u64>,
    bundle: impl Read,
) -> anyhow::Result<()> {
    for entry in &manifest.entries {
        let made = match &entry.kind {
            EntryKind::Dir => staging.make_dir(&entry.path),
            EntryKind::File { .. } if !wanted.contains_key(entry.path.as_str()) => {
                staging.copy_from_live(live, &entry.path)
            }
            EntryKind::File { .. } | EntryKind::Symlink { .. } => Ok(()),
        };
        if let Err(e) = made {
            return Err(e);
        }
    }
    if let Err(e) = receive_bundle(staging, wanted, bundle) {
        return Err(e);
    }
    for entry in &manifest.entries {
        let finished = match &entry.kind {
            EntryKind::Symlink { target } => staging.symlink(&entry.path, target),
            EntryKind::Dir | EntryKind::File { .. } => Ok(()),
        };
        if let Err(e) = finished {
            return Err(e);
        }
    }
    for entry in &manifest.entries {
        if let EntryKind::File { size, sha256, mode } = &entry.kind {
            if let Err(e) = staging.verify(&entry.path, *size, sha256, *mode) {
                return Err(e);
            }
        }
    }
    let listed: HashSet<&str> = manifest
        .entries
        .iter()
        .map(|entry| entry.path.as_str())
        .collect();
    staging.ensure_only(&listed)
}

#[cfg(test)]
#[path = "apply_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "apply_hostile_tests.rs"]
mod hostile_tests;

#[cfg(test)]
#[path = "apply_limits_tests.rs"]
mod limits_tests;
