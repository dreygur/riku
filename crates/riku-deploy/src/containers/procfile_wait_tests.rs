use std::fs;
use std::process::{Child, Command};
use std::thread;
use std::time::{Duration, Instant};

use crate::config::RikuPaths;
use crate::containers::mock_state::{mock_server, MockServer};
use crate::containers::mock_workers::procfile_workers;
use crate::containers::procfile_workers::ProcfileWorkers;
use crate::supervisor::process::pid_file::{worker_pid_dir, WorkerPidFile};

const LIMIT: Duration = Duration::from_secs(3);

/// A running supervisor as the wait sees one: its pid file, locked. The
/// lock lasts as long as the returned file.
fn write_supervisor_pid(paths: &RikuPaths) -> fs::File {
    let path = paths.riku_root.join("supervisor.pid");
    fs::write(&path, std::process::id().to_string()).unwrap();
    let lock = fs::File::open(&path).unwrap();
    lock.lock().unwrap();
    lock
}

fn procfile_server() -> (tempfile::TempDir, MockServer, ProcfileWorkers) {
    let tmp = tempfile::tempdir().unwrap();
    let server = mock_server(tmp.path(), "myapp", "", "");
    let _old = procfile_workers(&server);
    let workers = ProcfileWorkers::load("myapp", &server.paths).unwrap();
    (tmp, server, workers)
}

/// A long sleep standing in for a worker, with the marker the supervisor
/// would have written for it.
fn running_worker(paths: &RikuPaths) -> (Child, WorkerPidFile) {
    let child = Command::new("sleep").arg("30").spawn().unwrap();
    let dir = worker_pid_dir(&paths.riku_root);
    let marker = WorkerPidFile::create(&dir, "myapp@web.1", child.id()).unwrap();
    (child, marker)
}

#[test]
fn no_running_supervisor_means_nothing_to_wait_for() {
    let (_tmp, server, workers) = procfile_server();

    let outcome = workers.wait_stopped_within(&server.paths, LIMIT);

    assert!(
        outcome.is_ok(),
        "no supervisor, nothing to wait for: {outcome:?}"
    );
}

#[test]
fn returns_as_soon_as_the_supervisor_reaps_the_worker() {
    let (_tmp, server, workers) = procfile_server();
    let _supervisor = write_supervisor_pid(&server.paths);
    let (mut child, marker) = running_worker(&server.paths);
    let reaper = thread::spawn(move || {
        thread::sleep(Duration::from_millis(300));
        child.kill().unwrap();
        child.wait().unwrap();
        drop(marker);
    });

    let start = Instant::now();
    let outcome = workers.wait_stopped_within(&server.paths, LIMIT);
    let waited = start.elapsed();
    reaper.join().unwrap();

    assert!(outcome.is_ok(), "reaped worker must release: {outcome:?}");
    assert!(
        waited < Duration::from_secs(1),
        "waited {waited:?} for a worker reaped after 300 ms"
    );
}

#[test]
fn live_worker_times_out_with_the_app_named() {
    let (_tmp, server, workers) = procfile_server();
    let _supervisor = write_supervisor_pid(&server.paths);
    let (mut child, _marker) = running_worker(&server.paths);

    let outcome = workers.wait_stopped_within(&server.paths, Duration::from_millis(300));

    let _ = child.kill();
    let _ = child.wait();
    let message = format!(
        "{:#}",
        outcome.expect_err("a live worker must block the wait")
    );
    assert!(
        message.contains("old workers of 'myapp' did not stop"),
        "unclear timeout error: {message}"
    );
}
