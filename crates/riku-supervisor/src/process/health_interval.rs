//! Spacing of health probes by the configured `interval`.
//!
//! The monitor ticks every second; a process is probed only when its last
//! probe is at least `interval` seconds old.

use std::time::{Duration, Instant};

use super::ProcessManager;

fn is_due(last_probe: Option<Instant>, interval: Duration) -> bool {
    match last_probe {
        Some(at) => at.elapsed() >= interval,
        None => true,
    }
}

impl ProcessManager {
    /// Returns true and records the time when `process_id` is due a probe.
    pub(super) fn claim_health_probe(&mut self, process_id: &str, interval_secs: u64) -> bool {
        let process = match self.processes.get_mut(process_id) {
            Some(p) => p,
            None => return false,
        };
        if !is_due(
            process.last_health_probe,
            Duration::from_secs(interval_secs),
        ) {
            return false;
        }
        process.last_health_probe = Some(Instant::now());
        true
    }
}

#[cfg(test)]
#[path = "health_interval_tests.rs"]
mod tests;
