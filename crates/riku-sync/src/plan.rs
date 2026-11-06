//! Receiver side: decides which files to ask for and which live paths go away.
//!
//! The live tree is listed without following symlinks and without failing on
//! entries the running app may have created (sockets, fifos): those are listed
//! as paths but never read. Names that are not valid UTF-8 or not a valid
//! [`RelPath`] cannot be named on the wire and are left out of the listing; they
//! disappear when the new tree is swapped in.

use crate::scan::hash_file;
use crate::wire::{EntryKind, Manifest, RelPath, TransferPlan};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

enum LiveItem {
    File { size: u64, path: PathBuf },
    Other,
}

/// Compares `incoming` with the tree at `live`. A missing `live` needs every
/// file. A change of mode alone is not a need, since apply sets modes from the
/// manifest.
pub fn plan_transfer(live: &Path, incoming: &Manifest) -> anyhow::Result<TransferPlan> {
    let mut listing = BTreeMap::new();
    if let Err(e) = list_live(live, "", &mut listing) {
        return Err(e);
    }
    let need = incoming
        .entries
        .iter()
        .filter_map(|entry| match &entry.kind {
            EntryKind::File { size, sha256, .. }
                if !is_unchanged(&listing, &entry.path, *size, sha256) =>
            {
                Some(entry.path.clone())
            }
            _ => None,
        })
        .collect();
    let listed: HashSet<&RelPath> = incoming.entries.iter().map(|entry| &entry.path).collect();
    let delete = listing
        .keys()
        .filter(|path| !listed.contains(path))
        .cloned()
        .collect();
    Ok(TransferPlan { need, delete })
}

fn is_unchanged(
    listing: &BTreeMap<RelPath, LiveItem>,
    path: &RelPath,
    size: u64,
    sha256: &str,
) -> bool {
    match listing.get(path) {
        Some(LiveItem::File {
            size: live_size,
            path: full,
        }) => *live_size == size && matches!(hash_file(full), Ok(hash) if hash == sha256),
        _ => false,
    }
}

fn list_live(
    dir: &Path,
    prefix: &str,
    out: &mut BTreeMap<RelPath, LiveItem>,
) -> anyhow::Result<()> {
    let listing = match fs::read_dir(dir) {
        Ok(listing) => listing,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotADirectory => return Ok(()),
        Err(e) => {
            return Err(anyhow::anyhow!(
                "cannot read directory {}: {e}",
                dir.display()
            ))
        }
    };
    for item in listing {
        let item = match item {
            Ok(item) => item,
            Err(e) => return Err(anyhow::anyhow!("cannot list {}: {e}", dir.display())),
        };
        let file_name = item.file_name();
        let Some(name) = file_name.to_str() else {
            continue;
        };
        let rel = if prefix.is_empty() {
            name.to_string()
        } else {
            format!("{prefix}/{name}")
        };
        let Ok(rel_path) = RelPath::try_from(rel.as_str()) else {
            continue;
        };
        let meta = match fs::symlink_metadata(item.path()) {
            Ok(meta) => meta,
            Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "cannot stat {}: {e}",
                    item.path().display()
                ))
            }
        };
        let file_type = meta.file_type();
        if file_type.is_file() {
            out.insert(
                rel_path,
                LiveItem::File {
                    size: meta.len(),
                    path: item.path(),
                },
            );
        } else if file_type.is_dir() {
            out.insert(rel_path, LiveItem::Other);
            if let Err(e) = list_live(&item.path(), &rel, out) {
                return Err(e);
            }
        } else {
            out.insert(rel_path, LiveItem::Other);
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "plan_tests.rs"]
mod tests;
