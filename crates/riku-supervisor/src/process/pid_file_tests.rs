use std::fs::File;

use super::{app_has_live_worker, WorkerPidFile};

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
