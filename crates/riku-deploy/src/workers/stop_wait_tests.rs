use std::thread;
use std::time::{Duration, Instant};

use super::wait_stopped;
use crate::config::RikuPaths;
use crate::supervisor::process::pid_file::{worker_pid_dir, WorkerPidFile};

#[test]
fn stop_waits_until_the_copy_pid_file_is_gone() {
    let dir = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(dir.path());
    let pid_dir = worker_pid_dir(&paths.riku_root);
    let marker = WorkerPidFile::create(&pid_dir, "myapp@web.1", std::process::id()).unwrap();
    let other = WorkerPidFile::create(&pid_dir, "myapp@web.2", std::process::id()).unwrap();

    let supervisor = thread::spawn(move || {
        thread::sleep(Duration::from_millis(300));
        drop(marker);
    });
    let started = Instant::now();
    wait_stopped(&paths, "myapp@web.1", Duration::from_secs(5));
    let elapsed = started.elapsed();
    supervisor.join().unwrap();
    drop(other);

    assert!(
        elapsed >= Duration::from_millis(300),
        "returned while the pid file was still there: {elapsed:?}"
    );
    assert!(
        elapsed < Duration::from_secs(3),
        "kept waiting on another copy's pid file: {elapsed:?}"
    );
}
