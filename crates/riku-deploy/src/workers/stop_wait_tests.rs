use std::fs;
use std::thread;
use std::time::{Duration, Instant};

use chrono::Utc;
use riku_supervisor::stats::types::ProcessStats;
use riku_supervisor::stats::{AppStats, ProcessStatus};

use super::wait_stopped;
use crate::config::RikuPaths;

fn stats_json(status: ProcessStatus) -> String {
    let process = ProcessStats {
        process_id: "myapp-web-1".to_string(),
        app: "myapp".to_string(),
        kind: "web".to_string(),
        ordinal: 1,
        status,
        ..Default::default()
    };
    let app = AppStats {
        app: "myapp".to_string(),
        total_processes: 1,
        running_processes: 1,
        healthy_processes: 1,
        total_restarts: 0,
        total_memory_bytes: 0,
        total_cpu_time_ms: 0,
        processes: vec![process],
        last_updated: Utc::now(),
    };
    serde_json::to_string(&vec![app]).unwrap()
}

#[test]
fn stop_waits_until_stats_show_the_copy_gone() {
    let dir = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(dir.path());
    fs::create_dir_all(&paths.riku_root).unwrap();
    let stats_path = paths.riku_root.join("stats.json");
    fs::write(&stats_path, stats_json(ProcessStatus::Running)).unwrap();

    let writer = thread::spawn(move || {
        thread::sleep(Duration::from_millis(300));
        fs::write(stats_path, stats_json(ProcessStatus::Stopped)).unwrap();
    });
    let started = Instant::now();
    wait_stopped(&paths, "myapp-web-1", Duration::from_secs(5));
    let elapsed = started.elapsed();
    writer.join().unwrap();

    assert!(
        elapsed >= Duration::from_millis(300),
        "returned before the stats said stopped: {elapsed:?}"
    );
    assert!(
        elapsed < Duration::from_secs(3),
        "kept waiting after the stats said stopped: {elapsed:?}"
    );
}
