//! Helpers shared by the unit tests: tree builders, hand made tar bundles and
//! the fixture the apply tests run against.

use crate::apply::apply;
use crate::bundle::write_bundle;
use crate::plan::plan_transfer;
use crate::scan::scan;
use crate::wire::{Entry, EntryKind, Manifest, RelPath, TransferPlan};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Cursor;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// What the parent of `live` holds after any apply: the tree and its lock file.
pub const SETTLED: [&str; 2] = ["live", "live.sync-lock"];

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

pub fn manifest_of(entries: Vec<Entry>) -> Manifest {
    Manifest { entries }
}

pub fn plan_needing(paths: &[&str]) -> TransferPlan {
    TransferPlan {
        need: paths.iter().map(|path| rel(path)).collect(),
        delete: Vec::new(),
    }
}

pub fn names_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|item| item.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// A zstd compressed tar assembled by `build`, which may append entries that
/// the tar builder's own path setter would refuse.
pub fn bundle_from(build: impl FnOnce(&mut tar::Builder<Vec<u8>>)) -> Vec<u8> {
    let mut builder = tar::Builder::new(Vec::new());
    build(&mut builder);
    let tar_bytes = builder.into_inner().unwrap();
    zstd::encode_all(tar_bytes.as_slice(), 3).unwrap()
}

/// Appends an entry whose name and link name are written verbatim.
pub fn append_raw(
    builder: &mut tar::Builder<Vec<u8>>,
    kind: tar::EntryType,
    name: &str,
    link: &str,
    body: &[u8],
) {
    let mut header = tar::Header::new_gnu();
    header.as_old_mut().name[..name.len()].copy_from_slice(name.as_bytes());
    header.as_old_mut().linkname[..link.len()].copy_from_slice(link.as_bytes());
    header.set_size(body.len() as u64);
    header.set_mode(0o644);
    header.set_entry_type(kind);
    header.set_cksum();
    builder.append(&header, body).unwrap();
}

/// A bundle of regular files with verbatim names.
pub fn raw_bundle(entries: &[(&str, &[u8])]) -> Vec<u8> {
    bundle_from(|builder| {
        for (name, body) in entries {
            append_raw(builder, tar::EntryType::Regular, name, "", body);
        }
    })
}

pub struct Fixture {
    pub parent: TempDir,
    pub live: PathBuf,
}

pub fn fixture() -> Fixture {
    let parent = tempfile::tempdir().unwrap();
    let live = parent.path().join("live");
    put(&live, "keep.txt", "k");
    put(&live, "old.txt", "x");
    put(&live, "dir/inner.txt", "i");
    Fixture { parent, live }
}

/// Runs a hostile apply and checks the refusal left no trace: live is byte for
/// byte as before and its parent holds nothing but `live` and the lock file.
/// Returns the error text.
pub fn assert_refused_untouched(
    f: &Fixture,
    manifest: &Manifest,
    plan: &TransferPlan,
    bundle: Vec<u8>,
) -> String {
    let before = scan(&f.live).unwrap();
    let err = apply(&f.live, manifest, plan, Cursor::new(bundle)).unwrap_err();
    assert_eq!(
        scan(&f.live).unwrap(),
        before,
        "live changed after refusal: {err}"
    );
    assert_eq!(
        names_in(f.parent.path()),
        SETTLED,
        "stray directory after refusal: {err}"
    );
    err.to_string()
}

pub fn sync_from(src: &Path, live: &Path) {
    let manifest = scan(src).unwrap();
    let plan = plan_transfer(live, &manifest).unwrap();
    let mut bundle = Vec::new();
    write_bundle(src, &plan.need, &mut bundle).unwrap();
    apply(live, &manifest, &plan, Cursor::new(bundle)).unwrap();
}

pub fn tree_to_sync() -> TempDir {
    let src = tempfile::tempdir().unwrap();
    put(src.path(), "keep.txt", "k");
    put(src.path(), "app/main.py", "print()");
    put(src.path(), "bin/run", "#!/bin/sh\n");
    fs::set_permissions(
        src.path().join("bin/run"),
        fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    fs::create_dir_all(src.path().join("logs")).unwrap();
    symlink("/usr/bin/python3", src.path().join("python")).unwrap();
    src
}
