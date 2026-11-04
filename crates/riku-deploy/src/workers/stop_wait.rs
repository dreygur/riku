//! Waiting for the supervisor to finish stopping one worker copy.
//!
//! The supervisor stops a copy when its enabled config disappears and
//! removes the copy's pid file once the process and its group are gone.
//! Nothing here can fail a deploy: when the limit passes the supervisor
//! still kills the process after its grace period on its own.

use std::thread::sleep;
use std::time::{Duration, Instant};

use crate::config::RikuPaths;
use crate::supervisor::process::pid_file::{process_has_live_worker, worker_pid_dir};

#[cfg(not(test))]
const STOP_POLL: Duration = Duration::from_millis(200);
#[cfg(test)]
const STOP_POLL: Duration = Duration::from_millis(20);

/// Polls the supervisor's pid files until `process_id` no longer runs or
/// `limit` passes.
pub(super) fn wait_stopped(paths: &RikuPaths, process_id: &str, limit: Duration) {
    let pid_dir = worker_pid_dir(&paths.riku_root);
    let deadline = Instant::now() + limit;
    loop {
        match process_has_live_worker(&pid_dir, process_id) {
            Ok(false) => return,
            Ok(true) => {}
            Err(e) => {
                tracing::warn!(process_id, "cannot read {}: {e}", pid_dir.display());
                return;
            }
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
