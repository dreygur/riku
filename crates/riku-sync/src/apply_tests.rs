use super::*;
use crate::bundle::write_bundle;
use crate::plan::plan_transfer;
use crate::scan::scan;
use crate::testkit::{dir_entry, file_entry, put, raw_bundle, rel};
use crate::wire::{Entry, EntryKind};
use std::fs;
use std::io::Cursor;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::PathBuf;
use tempfile::TempDir;

struct Fixture {
    parent: TempDir,
    live: PathBuf,
}

fn fixture() -> Fixture {
    let parent = tempfile::tempdir().unwrap();
    let live = parent.path().join("live");
    put(&live, "keep.txt", "k");
    put(&live, "old.txt", "x");
    put(&live, "dir/inner.txt", "i");
    Fixture { parent, live }
}

fn names_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|item| item.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

fn plan_needing(paths: &[&str]) -> TransferPlan {
    TransferPlan {
        need: paths.iter().map(|path| rel(path)).collect(),
        delete: Vec::new(),
    }
}

fn manifest_of(entries: Vec<Entry>) -> Manifest {
    Manifest { entries }
}

/// Runs a hostile apply and checks the refusal left no trace: live is byte for
/// byte as before and its parent holds nothing but `live`.
fn assert_refused_untouched(
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
        ["live"],
        "stray directory after refusal: {err}"
    );
    err.to_string()
}

fn sync_from(src: &Path, live: &Path) {
    let manifest = scan(src).unwrap();
    let plan = plan_transfer(live, &manifest).unwrap();
    let mut bundle = Vec::new();
    write_bundle(src, &plan.need, &mut bundle).unwrap();
    apply(live, &manifest, &plan, Cursor::new(bundle)).unwrap();
}

fn tree_to_sync() -> TempDir {
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

#[test]
fn apply_makes_live_match_the_source() {
    let f = fixture();
    let src = tree_to_sync();

    sync_from(src.path(), &f.live);

    assert_eq!(scan(&f.live).unwrap(), scan(src.path()).unwrap());
    assert_eq!(names_in(f.parent.path()), ["live"]);
}

#[test]
fn first_sync_creates_a_missing_live_directory() {
    let parent = tempfile::tempdir().unwrap();
    let live = parent.path().join("live");
    let src = tree_to_sync();

    sync_from(src.path(), &live);

    assert_eq!(scan(&live).unwrap(), scan(src.path()).unwrap());
    assert_eq!(names_in(parent.path()), ["live"]);
}

#[test]
fn leftovers_of_a_crashed_apply_are_removed() {
    let f = fixture();
    put(&f.parent.path().join("live.sync-new"), "half.txt", "h");
    put(&f.parent.path().join("live.sync-old"), "older.txt", "o");
    let src = tree_to_sync();

    sync_from(src.path(), &f.live);

    assert_eq!(scan(&f.live).unwrap(), scan(src.path()).unwrap());
    assert_eq!(names_in(f.parent.path()), ["live"]);
}

#[test]
fn live_lost_between_the_two_renames_is_restored_first() {
    let parent = tempfile::tempdir().unwrap();
    let live = parent.path().join("live");
    put(&parent.path().join("live.sync-old"), "keep.txt", "k");
    put(&parent.path().join("live.sync-new"), "half.txt", "h");
    let manifest = manifest_of(vec![file_entry("keep.txt", "k")]);

    apply(
        &live,
        &manifest,
        &TransferPlan::default(),
        Cursor::new(raw_bundle(&[])),
    )
    .unwrap();

    assert_eq!(fs::read_to_string(live.join("keep.txt")).unwrap(), "k");
    assert_eq!(names_in(parent.path()), ["live"]);
}

#[test]
fn path_with_dotdot_in_the_stream_is_refused() {
    let f = fixture();
    let manifest = manifest_of(vec![file_entry("ok.txt", "abc")]);
    let bundle = raw_bundle(&[("../escape.txt", b"abc")]);

    let err = assert_refused_untouched(&f, &manifest, &plan_needing(&["ok.txt"]), bundle);

    assert!(err.contains("escape.txt"), "error was: {err}");
}

#[test]
fn file_under_a_symlinked_dir_is_refused() {
    let f = fixture();
    let outside = tempfile::tempdir().unwrap();
    let manifest = manifest_of(vec![
        Entry {
            path: rel("lib"),
            kind: EntryKind::Symlink {
                target: outside.path().to_string_lossy().into_owned(),
            },
        },
        file_entry("lib/passwd", "abc"),
    ]);
    let bundle = raw_bundle(&[("lib/passwd", b"abc")]);

    let err = assert_refused_untouched(&f, &manifest, &plan_needing(&["lib/passwd"]), bundle);

    assert!(err.contains("lib"), "error was: {err}");
    assert!(
        names_in(outside.path()).is_empty(),
        "wrote through the symlink"
    );
}

#[test]
fn live_file_behind_a_live_symlink_is_not_copied() {
    let f = fixture();
    let outside = tempfile::tempdir().unwrap();
    put(outside.path(), "secret", "s");
    symlink(outside.path(), f.live.join("lib")).unwrap();
    let manifest = manifest_of(vec![dir_entry("lib"), file_entry("lib/secret", "s")]);

    let err = assert_refused_untouched(&f, &manifest, &TransferPlan::default(), raw_bundle(&[]));

    assert!(err.contains("lib/secret"), "error was: {err}");
}

#[test]
fn oversized_file_is_refused() {
    let f = fixture();
    let manifest = manifest_of(vec![file_entry("big.txt", "abc")]);
    let bundle = raw_bundle(&[("big.txt", b"0123456789")]);

    let err = assert_refused_untouched(&f, &manifest, &plan_needing(&["big.txt"]), bundle);

    assert!(err.contains("big.txt"), "error was: {err}");
}

#[test]
fn unrequested_path_in_bundle_is_refused() {
    let f = fixture();
    let manifest = manifest_of(vec![file_entry("a.txt", "a")]);
    let bundle = raw_bundle(&[("a.txt", b"a"), ("extra.txt", b"x")]);

    let err = assert_refused_untouched(&f, &manifest, &plan_needing(&["a.txt"]), bundle);

    assert!(err.contains("extra.txt"), "error was: {err}");
}

#[test]
fn hash_mismatch_is_refused() {
    let f = fixture();
    let manifest = manifest_of(vec![file_entry("a.txt", "abc")]);
    let bundle = raw_bundle(&[("a.txt", b"abd")]);

    let err = assert_refused_untouched(&f, &manifest, &plan_needing(&["a.txt"]), bundle);

    assert!(
        err.contains("a.txt") && err.contains("sha256"),
        "error was: {err}"
    );
}

#[test]
fn stream_cut_short_is_refused() {
    let src = tempfile::tempdir().unwrap();
    let body = |seed: u32| {
        (0..6000u32)
            .map(|i| format!("{:x}", i.wrapping_mul(seed) ^ seed))
            .collect::<String>()
    };
    put(src.path(), "one.txt", &body(2654435761));
    put(src.path(), "two.txt", &body(40503));
    let manifest = scan(src.path()).unwrap();
    let plan = plan_needing(&["one.txt", "two.txt"]);
    let mut whole = Vec::new();
    write_bundle(src.path(), &plan.need, &mut whole).unwrap();

    for cut in [whole.len() / 2, whole.len() - 1] {
        let f = fixture();
        assert_refused_untouched(&f, &manifest, &plan, whole[..cut].to_vec());
    }
}

#[test]
fn unchecked_manifest_with_an_unlisted_parent_is_refused() {
    let f = fixture();
    let manifest = manifest_of(vec![file_entry("sub/x.txt", "x")]);
    let bundle = raw_bundle(&[("sub/x.txt", b"x")]);

    let err = assert_refused_untouched(&f, &manifest, &plan_needing(&["sub/x.txt"]), bundle);

    assert!(err.contains("sub"), "error was: {err}");
}

#[test]
fn malformed_sha256_is_refused_plainly() {
    let f = fixture();
    let manifest = manifest_of(vec![Entry {
        path: rel("a.txt"),
        kind: EntryKind::File {
            size: 1,
            sha256: "not-hex".to_string(),
            mode: 0o644,
        },
    }]);
    let bundle = raw_bundle(&[("a.txt", b"a")]);

    let err = assert_refused_untouched(&f, &manifest, &plan_needing(&["a.txt"]), bundle);

    assert!(err.contains("malformed sha256"), "error was: {err}");
}
