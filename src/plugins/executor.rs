//! Plugin process execution helpers.
//!
//! Low-level utilities for spawning, timing out, and capturing output
//! from plugin child processes. Used exclusively by [`super::manager`].

use std::io::{BufRead, BufReader};
use std::time::{Duration, Instant};

/// Read plugin timeout from `RIKU_PLUGIN_TIMEOUT` env var (seconds).
/// Defaults to 300 seconds (5 minutes).
pub(super) fn plugin_timeout() -> Duration {
    std::env::var("RIKU_PLUGIN_TIMEOUT")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .map(Duration::from_secs)
        .unwrap_or(Duration::from_secs(300))
}

/// Poll child every 200ms until it exits or the timeout elapses.
/// Kills the child (and reaps it) on timeout. Returns `true` if timed out.
pub(super) fn wait_with_timeout(child: &mut std::process::Child, timeout: Duration) -> bool {
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return false, // exited normally
            Ok(None) if start.elapsed() >= timeout => {
                child.kill().ok();
                child.wait().ok(); // reap to avoid zombie
                return true;
            }
            _ => std::thread::sleep(Duration::from_millis(200)),
        }
    }
}

/// Emit captured stdout as INFO and stderr as WARN via tracing.
pub(super) fn emit_plugin_output(child: &mut std::process::Child, plugin_name: &str) {
    if let Some(stdout) = child.stdout.take() {
        for line in BufReader::new(stdout).lines().flatten() {
            tracing::info!(plugin = plugin_name, "{}", line);
        }
    }
    if let Some(stderr) = child.stderr.take() {
        for line in BufReader::new(stderr).lines().flatten() {
            tracing::warn!(plugin = plugin_name, "{}", line);
        }
    }
}
