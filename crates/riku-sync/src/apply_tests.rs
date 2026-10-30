use super::*;
use crate::lock;
use crate::scan::scan;
use crate::testkit::*;
use std::fs;
use std::io::Cursor;

#[test]
fn apply_makes_live_match_the_source() {
    let f = fixture();
    let src = tree_to_sync();

    sync_from(src.path(), &f.live);

    assert_eq!(scan(&f.live).unwrap(), scan(src.path()).unwrap());
    assert_eq!(names_in(f.parent.path()), SETTLED);
}

#[test]
fn first_sync_creates_a_missing_live_directory() {
    let parent = tempfile::tempdir().unwrap();
    let live = parent.path().join("live");
    let src = tree_to_sync();

    sync_from(src.path(), &live);

    assert_eq!(scan(&live).unwrap(), scan(src.path()).unwrap());
    assert_eq!(names_in(parent.path()), SETTLED);
}

#[test]
fn leftovers_of_a_crashed_apply_are_removed() {
    let f = fixture();
    put(&f.parent.path().join("live.sync-new"), "half.txt", "h");
    put(&f.parent.path().join("live.sync-old"), "older.txt", "o");
    let src = tree_to_sync();

    sync_from(src.path(), &f.live);

    assert_eq!(scan(&f.live).unwrap(), scan(src.path()).unwrap());
    assert_eq!(names_in(f.parent.path()), SETTLED);
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
    assert_eq!(names_in(parent.path()), SETTLED);
}

#[test]
fn second_apply_is_refused_while_the_first_holds_the_lock() {
    let f = fixture();
    let before = scan(&f.live).unwrap();
    let running = lock::acquire(&f.live).unwrap();
    let src = tree_to_sync();
    let manifest = scan(src.path()).unwrap();

    let err = apply(
        &f.live,
        &manifest,
        &TransferPlan::default(),
        Cursor::new(raw_bundle(&[])),
    )
    .unwrap_err();

    assert!(err.to_string().contains("another sync"), "error was: {err}");
    assert_eq!(scan(&f.live).unwrap(), before);
    assert_eq!(
        names_in(f.parent.path()),
        SETTLED,
        "second apply left debris"
    );
    drop(running);
}
