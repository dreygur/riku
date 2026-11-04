use std::fs::File;

use super::{app_has_live_worker, clear_markers, WorkerPidFile};

#[test]
fn startup_clear_removes_markers_a_dead_supervisor_left() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("pids");
    let leftover = WorkerPidFile::create(&dir, "myapp@web.1", std::process::id()).unwrap();
    std::mem::forget(leftover);

    clear_markers(&dir).unwrap();

    let left: Vec<_> = std::fs::read_dir(&dir).unwrap().collect();
    assert!(left.is_empty(), "markers survived the clear: {left:?}");
}

#[test]
fn live_worker_is_seen_until_its_marker_is_dropped() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("pids");
    let marker = WorkerPidFile::create(&dir, "myapp@web.1", std::process::id()).unwrap();

    assert!(
        app_has_live_worker(&dir, "myapp").unwrap(),
        "marker present"
    );
    assert!(
        !app_has_live_worker(&dir, "other").unwrap(),
        "a marker of myapp must not count for another app"
    );

    drop(marker);
    assert!(
        !app_has_live_worker(&dir, "myapp").unwrap(),
        "dropping the marker must remove it"
    );
}

#[test]
fn marker_left_by_a_dead_process_is_ignored() {
    let tmp = tempfile::tempdir().unwrap();
    let mut child = std::process::Command::new("true").spawn().unwrap();
    let pid = child.id();
    child.wait().unwrap();
    File::create(tmp.path().join(format!("myapp@web.1.{pid}"))).unwrap();

    assert!(
        !app_has_live_worker(tmp.path(), "myapp").unwrap(),
        "pid {pid} has exited, its stale marker must not block"
    );
}
