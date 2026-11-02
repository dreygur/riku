use std::fs;
use std::time::{Duration, SystemTime};

use chrono::Utc;

use crate::config::RikuPaths;
use crate::containers::legacy_workers::LegacyWorkers;
use crate::containers::mock_legacy::legacy_stack;
use crate::containers::mock_state::{mock_server, MockServer};
use crate::supervisor::stats::{AppStats, ProcessStats, ProcessStatus};

const LIMIT: Duration = Duration::from_millis(300);

fn write_supervisor_pid(paths: &RikuPaths) {
    let path = paths.riku_root.join("supervisor.pid");
    fs::write(path, std::process::id().to_string()).unwrap();
}

fn worker(status: ProcessStatus, pid: Option<u32>) -> ProcessStats {
    ProcessStats {
        process_id: "myapp-web-1".to_string(),
        app: "myapp".to_string(),
        kind: "web".to_string(),
        ordinal: 1,
        pid,
        status,
        ..Default::default()
    }
}

fn write_stats(paths: &RikuPaths, process: ProcessStats) {
    let running = u32::from(process.status == ProcessStatus::Running);
    let stats = vec![AppStats {
        app: "myapp".to_string(),
        total_processes: 1,
        running_processes: running,
        healthy_processes: 0,
        total_restarts: 0,
        total_memory_bytes: 0,
        total_cpu_time_ms: 0,
        processes: vec![process],
        last_updated: Utc::now(),
    }];
    fs::write(
        paths.riku_root.join("stats.json"),
        serde_json::to_string(&stats).unwrap(),
    )
    .unwrap();
}

fn set_stats_mtime(paths: &RikuPaths, mtime: SystemTime) {
    let file = fs::OpenOptions::new()
        .write(true)
        .open(paths.riku_root.join("stats.json"))
        .unwrap();
    file.set_modified(mtime).unwrap();
}

fn legacy_server() -> (tempfile::TempDir, MockServer, LegacyWorkers) {
    let tmp = tempfile::tempdir().unwrap();
    let server = mock_server(tmp.path(), "myapp", "", "");
    let _old = legacy_stack(&server);
    let legacy = LegacyWorkers::load("myapp", &server.paths).unwrap();
    (tmp, server, legacy)
}

#[test]
fn no_running_supervisor_means_nothing_to_wait_for() {
    let (_tmp, server, legacy) = legacy_server();

    let outcome = legacy.wait_stopped_since(&server.paths, SystemTime::now(), LIMIT);

    assert!(
        outcome.is_ok(),
        "no supervisor, nothing to wait for: {outcome:?}"
    );
}

#[test]
fn stop_wait_ignores_stats_older_than_the_park() {
    let (_tmp, server, legacy) = legacy_server();
    write_supervisor_pid(&server.paths);
    let parked_at = SystemTime::now();
    write_stats(&server.paths, worker(ProcessStatus::Stopped, None));
    set_stats_mtime(&server.paths, parked_at - Duration::from_secs(10));

    let outcome = legacy.wait_stopped_since(&server.paths, parked_at, LIMIT);

    assert!(
        outcome.is_err(),
        "a snapshot older than the park proves nothing"
    );
}

#[test]
fn running_worker_blocks_until_snapshot_shows_it_stopped() {
    let (_tmp, server, legacy) = legacy_server();
    write_supervisor_pid(&server.paths);
    let parked_at = SystemTime::now();
    let mut child = std::process::Command::new("sleep")
        .arg("5")
        .spawn()
        .unwrap();
    write_stats(
        &server.paths,
        worker(ProcessStatus::Running, Some(child.id())),
    );

    let blocked = legacy.wait_stopped_since(&server.paths, parked_at, LIMIT);

    let _ = child.kill();
    let _ = child.wait();
    assert!(blocked.is_err(), "a running worker must block the wait");

    write_stats(&server.paths, worker(ProcessStatus::Stopped, None));
    let released = legacy.wait_stopped_since(&server.paths, parked_at, LIMIT);
    assert!(
        released.is_ok(),
        "stopped snapshot must release: {released:?}"
    );
}
