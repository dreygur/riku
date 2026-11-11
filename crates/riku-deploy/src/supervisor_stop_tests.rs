use std::fs::{self, File};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use super::*;
use crate::fake_supervisor::FakeSupervisor;

/// A `<proc>/<pid>/stat` line whose start time is `start`, with a command
/// name holding a space and a parenthesis, as the kernel allows.
fn fake_stat(proc_root: &Path, pid: u32, start: u64) {
    let dir = proc_root.join(pid.to_string());
    fs::create_dir_all(&dir).unwrap();
    let fields_before_start = "S 1 1 1 0 -1 0 0 0 0 0 0 0 0 0 20 0 1 0";
    fs::write(
        dir.join("stat"),
        format!("{pid} (riku (x) y) {fields_before_start} {start} 0 0\n"),
    )
    .unwrap();
}

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
    fs::write(&pid_file, child.id().to_string()).unwrap();
    let proc_root = tmp.path().join("proc");
    let seen = seen_supervisor(&paths, &proc_root).expect("the sleep holds the lock");

    let stopped = stop_supervisor(&paths, &seen, &proc_root, Duration::from_secs(10));
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
    let proc_root = tmp.path().join("proc");
    let seen = seen_supervisor(&paths, &proc_root).unwrap();
    assert_eq!(seen.pid, fake.child.id() as i32);

    let stopped = stop_supervisor(&paths, &seen, &proc_root, Duration::from_millis(300));

    assert!(!stopped, "the lock is still held after the wait");
}

/// The pid seen holding the lock started again in between (its start time
/// changed), so it now names another process: it must not be signalled.
#[test]
fn a_pid_whose_start_time_changed_is_not_signalled() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    let mut fake = FakeSupervisor::start(&paths);
    let proc_root = tmp.path().join("proc");
    fake_stat(&proc_root, fake.child.id(), 100);
    let seen = seen_supervisor(&paths, &proc_root).unwrap();
    fake_stat(&proc_root, fake.child.id(), 200);

    let stopped = stop_supervisor(&paths, &seen, &proc_root, Duration::from_millis(300));

    assert!(!stopped, "the lock is still held");
    assert!(
        fake.child.try_wait().unwrap().is_none(),
        "the process now behind that pid must not get SIGTERM"
    );
}
