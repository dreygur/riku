//! Statistics manager for tracking process metrics.

use chrono::Utc;
use std::collections::HashMap;
use std::fs;
use std::time::Instant;

use super::types::{HealthStatus, ProcessStats, ProcessStatus};

/// Statistics manager for tracking all process metrics.
pub struct StatsManager {
    pub(super) stats: HashMap<String, ProcessStats>,
    pub(super) start_times: HashMap<String, Instant>,
    #[allow(dead_code)]
    pub(super) request_counts: HashMap<String, u64>,
}

impl Default for StatsManager {
    fn default() -> Self {
        Self::new()
    }
}

impl StatsManager {
    /// Create a new stats manager.
    pub fn new() -> Self {
        StatsManager {
            stats: HashMap::new(),
            start_times: HashMap::new(),
            request_counts: HashMap::new(),
        }
    }

    /// Register a new process.
    pub fn register_process(
        &mut self,
        process_id: String,
        app: String,
        kind: String,
        ordinal: u32,
    ) {
        let stats = ProcessStats {
            process_id: process_id.clone(),
            app,
            kind,
            ordinal,
            started_at: Some(Utc::now()),
            status: ProcessStatus::Starting,
            ..Default::default()
        };

        self.stats.insert(process_id.clone(), stats);
        self.start_times.insert(process_id, Instant::now());
    }

    /// Mark a process as running.
    pub fn mark_running(&mut self, process_id: &str, pid: u32) {
        if let Some(stats) = self.stats.get_mut(process_id) {
            stats.status = ProcessStatus::Running;
            stats.pid = Some(pid);
        }
    }

    /// Mark a process as stopped.
    pub fn mark_stopped(&mut self, process_id: &str) {
        if let Some(stats) = self.stats.get_mut(process_id) {
            stats.status = ProcessStatus::Stopped;
        }
    }

    /// Mark a process as crashed.
    pub fn mark_crashed(&mut self, process_id: &str) {
        if let Some(stats) = self.stats.get_mut(process_id) {
            stats.status = ProcessStatus::Crashed;
        }
    }

    /// Mark a process as killed by the kernel OOM killer (cgroup
    /// `memory.max` exceeded).
    pub fn mark_oom_killed(&mut self, process_id: &str) {
        if let Some(stats) = self.stats.get_mut(process_id) {
            stats.status = ProcessStatus::OomKilled;
        }
    }

    /// Mark a process as restarting.
    pub fn mark_restarting(&mut self, process_id: &str) {
        if let Some(stats) = self.stats.get_mut(process_id) {
            stats.status = ProcessStatus::Restarting;
            stats.restart_count += 1;
            stats.last_restart_at = Some(Utc::now());
        }
    }

    /// Update health check status.
    pub fn update_health_check(&mut self, process_id: &str, status: HealthStatus) {
        if let Some(stats) = self.stats.get_mut(process_id) {
            stats.health_check_status = status;
            stats.last_health_check = Some(Utc::now());
        }
    }

    /// Update resource usage for a process.
    pub fn update_resource_usage(&mut self, process_id: &str, cpu_ms: u64, memory_bytes: u64) {
        if let Some(stats) = self.stats.get_mut(process_id) {
            stats.cpu_time_ms = cpu_ms;
            stats.memory_bytes = memory_bytes;
        }
    }

    /// Increment request count for a process.
    #[allow(dead_code)]
    pub fn increment_requests(&mut self, process_id: &str) {
        if let Some(stats) = self.stats.get_mut(process_id) {
            stats.requests_total += 1;

            // Calculate requests per second
            if let Some(start_time) = self.start_times.get(process_id) {
                let elapsed = start_time.elapsed().as_secs_f64();
                if elapsed > 0.0 {
                    stats.requests_per_second = stats.requests_total as f64 / elapsed;
                }
            }
        }

        *self
            .request_counts
            .entry(process_id.to_string())
            .or_insert(0) += 1;
    }

    /// Write stats to a JSON file for CLI consumption.
    pub fn write_stats_to_file(&self, path: &std::path::Path) -> Result<(), std::io::Error> {
        let app_stats = self.get_all_stats();
        let json = serde_json::to_string_pretty(&app_stats)
            .map_err(|e| std::io::Error::other(e.to_string()))?;

        // Write to temporary file first, then atomically rename
        // This prevents corruption if supervisor crashes mid-write
        let temp_path = path.with_extension("tmp");
        fs::write(&temp_path, json)?;

        // Atomic rename (guaranteed atomic on POSIX systems)
        fs::rename(&temp_path, path)?;
        Ok(())
    }
}
