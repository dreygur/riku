//! Helpers shared by the unit tests: tree builders and hand made tar bundles.

use crate::wire::{Entry, EntryKind, RelPath};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

pub fn rel(path: &str) -> RelPath {
    RelPath::try_from(path).unwrap()
}

pub fn put(root: &Path, path: &str, body: &str) {
    let full = root.join(path);
    fs::create_dir_all(full.parent().unwrap()).unwrap();
    fs::write(full, body).unwrap();
}

pub fn file_entry(path: &str, body: &str) -> Entry {
    Entry {
        path: rel(path),
        kind: EntryKind::File {
            size: body.len() as u64,
            sha256: hex::encode(Sha256::digest(body.as_bytes())),
            mode: 0o644,
        },
    }
}

pub fn dir_entry(path: &str) -> Entry {
    Entry {
        path: rel(path),
        kind: EntryKind::Dir,
    }
}

/// A zstd compressed tar whose entry names are written verbatim, so a test can
/// carry names (`../x`) that the tar builder's own path setter would refuse.
pub fn raw_bundle(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut builder = tar::Builder::new(Vec::new());
    for (name, body) in entries {
        let mut header = tar::Header::new_old();
        header.as_old_mut().name[..name.len()].copy_from_slice(name.as_bytes());
        header.set_size(body.len() as u64);
        header.set_mode(0o644);
        header.set_entry_type(tar::EntryType::Regular);
        header.set_cksum();
        builder.append(&header, *body).unwrap();
    }
    let tar_bytes = builder.into_inner().unwrap();
    zstd::encode_all(tar_bytes.as_slice(), 3).unwrap()
}
