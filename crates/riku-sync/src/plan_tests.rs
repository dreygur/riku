use super::*;
use crate::scan::scan;
use crate::testkit::put;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;

fn paths(list: &[RelPath]) -> Vec<&str> {
    list.iter().map(RelPath::as_str).collect()
}

#[test]
fn plan_needs_only_changed_files() {
    let live = tempfile::tempdir().unwrap();
    let src = tempfile::tempdir().unwrap();
    put(live.path(), "a.txt", "1");
    put(live.path(), "b.txt", "2");
    put(live.path(), "old.txt", "x");
    put(live.path(), "run.sh", "#!/bin/sh\n");
    put(src.path(), "a.txt", "1");
    put(src.path(), "b.txt", "3");
    put(src.path(), "c.txt", "4");
    put(src.path(), "run.sh", "#!/bin/sh\n");
    std::fs::set_permissions(
        src.path().join("run.sh"),
        std::fs::Permissions::from_mode(0o755),
    )
    .unwrap();

    let plan = plan_transfer(live.path(), &scan(src.path()).unwrap()).unwrap();

    assert_eq!(
        paths(&plan.need),
        ["b.txt", "c.txt"],
        "mode only change must not be needed"
    );
    assert_eq!(paths(&plan.delete), ["old.txt"]);
}

#[test]
fn missing_live_needs_every_file() {
    let parent = tempfile::tempdir().unwrap();
    let src = tempfile::tempdir().unwrap();
    put(src.path(), "app/main.py", "print()");
    put(src.path(), "top.txt", "t");

    let plan = plan_transfer(&parent.path().join("live"), &scan(src.path()).unwrap()).unwrap();

    assert_eq!(paths(&plan.need), ["app/main.py", "top.txt"]);
    assert!(plan.delete.is_empty());
}

#[test]
fn a_socket_in_live_does_not_fail_the_plan() {
    let live = tempfile::tempdir().unwrap();
    put(live.path(), "keep.txt", "k");
    let _listener = UnixListener::bind(live.path().join("app.sock")).unwrap();
    let src = tempfile::tempdir().unwrap();
    put(src.path(), "keep.txt", "k");

    let plan = plan_transfer(live.path(), &scan(src.path()).unwrap()).unwrap();

    assert!(plan.need.is_empty());
    assert_eq!(paths(&plan.delete), ["app.sock"]);
}
