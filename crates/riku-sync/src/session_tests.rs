use crate::scan;
use crate::session_kit::{changed_source, sync_over_sockets};
use crate::testkit::{fixture, names_in, put, rel};
use std::fs;

#[test]
fn session_plans_against_the_tree_recovered_from_a_crash() {
    let f = fixture();
    let src = changed_source();
    fs::rename(&f.live, f.parent.path().join("live.sync-old")).unwrap();
    fs::create_dir(f.parent.path().join("live.sync-new")).unwrap();

    let report = sync_over_sockets(src.path(), &f.live).unwrap();
    assert_eq!(
        report.sent,
        vec![rel("dir/inner.txt"), rel("new.txt")],
        "only changed files should be resent after recovery"
    );
    assert_eq!(report.deleted, vec![rel("old.txt")]);
    assert_eq!(scan(&f.live).unwrap(), scan(src.path()).unwrap());
    assert_eq!(
        names_in(f.parent.path()),
        ["live", "live.sync-lock"],
        "crash leftovers remain"
    );
}

#[test]
fn second_sync_moves_only_the_changed_file() {
    let src = tempfile::tempdir().unwrap();
    put(src.path(), "app.py", "v1");
    put(src.path(), "old.txt", "x");
    put(src.path(), "lib/util.py", "u");
    let parent = tempfile::tempdir().unwrap();
    let live = parent.path().join("live");

    let first = sync_over_sockets(src.path(), &live).unwrap();
    assert_eq!(
        first.sent,
        vec![rel("app.py"), rel("lib/util.py"), rel("old.txt")],
        "first sync must send every file"
    );
    assert!(first.bytes > 0, "no bytes counted for the first bundle");

    put(src.path(), "app.py", "v2");
    fs::remove_file(src.path().join("old.txt")).unwrap();
    let second = sync_over_sockets(src.path(), &live).unwrap();
    assert_eq!(second.sent, vec![rel("app.py")]);
    assert_eq!(second.deleted, vec![rel("old.txt")]);
    assert_eq!(scan(&live).unwrap(), scan(src.path()).unwrap());
}
