//! Waiting for the supervisor to report a stopped worker copy gone.
//!
//! The supervisor stops a copy when its enabled config disappears and records
//! the outcome in `<riku_root>/stats.json`. Nothing here can fail a deploy:
//! when the file is missing there is no supervisor to wait for, and when the
//! limit passes the supervisor kills the process after its grace period on
//! its own.

use std::thread::sleep;
use std::time::{Duration, Instant};

use riku_supervisor::stats::ProcessStatus;

use crate::config::RikuPaths;

#[cfg(not(test))]
const STOP_POLL: Duration = Duration::from_millis(200);
#[cfg(test)]
const STOP_POLL: Duration = Duration::from_millis(20);

fn copy_is_gone(paths: &RikuPaths, process_id: &str) -> bool {
    let stats_path = paths.riku_root.join("stats.json");
    let apps = match riku_supervisor::stats::read_stats_file(&stats_path) {
        Some(apps) => apps,
        None => return true,
    };
    !apps.iter().flat_map(|app| &app.processes).any(|process| {
        process.process_id == process_id
            && matches!(
                process.status,
                ProcessStatus::Running | ProcessStatus::Starting | ProcessStatus::Restarting
            )
    })
}

/// Polls the supervisor's stats until `process_id` no longer runs or `limit`
/// passes.
pub(super) fn wait_stopped(paths: &RikuPaths, process_id: &str, limit: Duration) {
    let deadline = Instant::now() + limit;
    loop {
        if copy_is_gone(paths, process_id) {
            return;
        }
        if Instant::now() >= deadline {
            tracing::warn!(process_id, "copy still running {limit:?} after its stop");
            return;
        }
        sleep(STOP_POLL);
    }
}

#[cfg(test)]
#[path = "stop_wait_tests.rs"]
mod tests;
