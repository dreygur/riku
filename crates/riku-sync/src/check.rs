//! Re-validation of a manifest and plan before the receiver writes anything.
//!
//! A manifest parsed from JSON is already checked, but one built in code is
//! not, so apply repeats the checks it relies on: unique paths, every parent a
//! directory listed earlier, sane modes and hashes, and a plan that asks only
//! for files the manifest lists.

use crate::wire::{EntryKind, Manifest, TransferPlan};
use std::collections::HashMap;

/// Returns the declared size of every file in `plan.need`, keyed by path.
pub(crate) fn wanted_sizes<'a>(
    manifest: &'a Manifest,
    plan: &'a TransferPlan,
) -> anyhow::Result<HashMap<&'a str, u64>> {
    let mut seen: HashMap<&str, bool> = HashMap::new();
    let mut sizes: HashMap<&str, u64> = HashMap::new();
    for entry in &manifest.entries {
        let path = entry.path.as_str();
        if let Some(parent) = entry.path.parent() {
            if seen.get(parent.as_str()) != Some(&true) {
                return Err(anyhow::anyhow!(
                    "manifest entry {path:?} has no directory {:?} listed above it",
                    parent.as_str()
                ));
            }
        }
        if seen
            .insert(path, matches!(entry.kind, EntryKind::Dir))
            .is_some()
        {
            return Err(anyhow::anyhow!("manifest lists {path:?} twice"));
        }
        if let EntryKind::File { size, sha256, mode } = &entry.kind {
            if !is_sha256_hex(sha256) {
                return Err(anyhow::anyhow!(
                    "manifest file {path:?} has a malformed sha256"
                ));
            }
            if mode & !0o777 != 0 {
                return Err(anyhow::anyhow!(
                    "manifest file {path:?} has mode {mode:o}, outside 0o777"
                ));
            }
            sizes.insert(path, *size);
        }
    }
    let mut wanted = HashMap::new();
    for needed in &plan.need {
        match sizes.get(needed.as_str()) {
            Some(size) => {
                wanted.insert(needed.as_str(), *size);
            }
            None => {
                return Err(anyhow::anyhow!(
                    "plan needs {:?}, which the manifest does not list as a file",
                    needed.as_str()
                ))
            }
        }
    }
    Ok(wanted)
}

fn is_sha256_hex(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}
