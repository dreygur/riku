//! Manifest scan of a directory tree.
//!
//! Security model: the scan reads only what lives under the given root and
//! never follows a symlink, so a link pointing outside the tree is recorded as
//! a link and its target is not read. Sockets, fifos and devices are refused
//! rather than skipped, so the receiver never silently gets a partial tree.

#![allow(clippy::question_mark)]

use crate::wire::{Entry, EntryKind, Manifest, RelPath};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

/// Scans `root` into a manifest sorted by path. The root itself is not listed
/// as an entry; its mode is the manifest's `root_mode`.
pub fn scan(root: &Path) -> anyhow::Result<Manifest> {
    let root_mode = match fs::metadata(root) {
        Ok(meta) => meta.permissions().mode() & 0o777,
        Err(e) => return Err(anyhow::anyhow!("cannot stat {}: {e}", root.display())),
    };
    let mut entries = Vec::new();
    if let Err(e) = walk(root, "", &mut entries) {
        return Err(e);
    }
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(Manifest { root_mode, entries })
}

fn walk(dir: &Path, prefix: &str, out: &mut Vec<Entry>) -> anyhow::Result<()> {
    let listing = match fs::read_dir(dir) {
        Ok(listing) => listing,
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
        let name = match file_name.to_str() {
            Some(name) => name,
            None => {
                return Err(anyhow::anyhow!(
                    "file name is not valid UTF-8: {}",
                    file_name.to_string_lossy()
                ))
            }
        };
        let rel = if prefix.is_empty() {
            name.to_string()
        } else {
            format!("{prefix}/{name}")
        };
        if let Err(e) = record(&item.path(), &rel, out) {
            return Err(e);
        }
    }
    Ok(())
}

fn record(path: &Path, rel: &str, out: &mut Vec<Entry>) -> anyhow::Result<()> {
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(e) => return Err(anyhow::anyhow!("cannot stat {}: {e}", path.display())),
    };
    let rel_path = match RelPath::try_from(rel) {
        Ok(rel_path) => rel_path,
        Err(e) => return Err(e),
    };
    let file_type = meta.file_type();
    if file_type.is_dir() {
        out.push(Entry {
            path: rel_path,
            kind: EntryKind::Dir {
                mode: meta.permissions().mode() & 0o777,
            },
        });
        return walk(path, rel, out);
    }
    if file_type.is_symlink() {
        let target = match fs::read_link(path) {
            Ok(target) => target,
            Err(e) => return Err(anyhow::anyhow!("cannot read link {}: {e}", path.display())),
        };
        let target = match target.to_str() {
            Some(target) => target.to_string(),
            None => {
                return Err(anyhow::anyhow!(
                    "symlink target is not valid UTF-8: {}",
                    path.display()
                ))
            }
        };
        out.push(Entry {
            path: rel_path,
            kind: EntryKind::Symlink { target },
        });
        return Ok(());
    }
    if !file_type.is_file() {
        return Err(anyhow::anyhow!(
            "unsupported file type (socket, fifo or device): {}",
            path.display()
        ));
    }
    let sha256 = match hash_file(path) {
        Ok(sha256) => sha256,
        Err(e) => return Err(e),
    };
    let kind = EntryKind::File {
        size: meta.len(),
        sha256,
        mode: meta.permissions().mode() & 0o777,
    };
    out.push(Entry {
        path: rel_path,
        kind,
    });
    Ok(())
}

pub(crate) fn hash_file(path: &Path) -> anyhow::Result<String> {
    let mut file = match fs::File::open(path) {
        Ok(file) => file,
        Err(e) => return Err(anyhow::anyhow!("cannot open {}: {e}", path.display())),
    };
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        match file.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => hasher.update(&buf[..n]),
            Err(e) => return Err(anyhow::anyhow!("cannot read {}: {e}", path.display())),
        }
    }
    Ok(hex::encode(hasher.finalize()))
}

#[cfg(test)]
#[path = "scan_tests.rs"]
mod tests;
