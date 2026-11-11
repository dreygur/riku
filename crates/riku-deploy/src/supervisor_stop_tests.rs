use std::fs::{self, File};
use std::process::{Command, Stdio};
use std::time::Duration;

use super::*;
use crate::fake_supervisor::FakeSupervisor;

/// A `sleep` holding the pid file lock through its stdin, so the lock frees
/// only when it exits, as a real supervisor's does.
#[test]
fn stopping_the_supervisor_waits_for_its_lock_to_free() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    fs::create_dir_all(&paths.riku_root).unwrap();
    let pid_file = paths.riku_root.join("supervisor.pid");
    fs::write(&pid_file, "").unwrap();
    let lock = File::open(&pid_file).unwrap();
    lock.lock().unwrap();
    let mut child = Command::new("sleep")
        .arg("30")
        .stdin(Stdio::from(lock))
        .spawn()
        .unwrap();

    let stopped = stop_supervisor(&paths, child.id() as i32, Duration::from_secs(10));
    let _ = child.kill();
    let _ = child.wait();

    assert!(stopped, "SIGTERM ends the sleep, which frees the lock");
    assert!(
        !pid_file_locked(&paths),
        "nothing may hold the lock afterwards"
    );
}

/// The lock here is held by the test, not by the signalled `sleep`, so it
/// never frees: the wait must give up instead of hanging init.
#[test]
fn stopping_gives_up_when_the_lock_stays_held() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    let fake = FakeSupervisor::start(&paths);

    let stopped = stop_supervisor(&paths, fake.child.id() as i32, Duration::from_millis(300));

    assert!(!stopped, "the lock is still held after the wait");
}
