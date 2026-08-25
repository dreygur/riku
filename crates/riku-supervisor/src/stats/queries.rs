//! Read-only query methods for the StatsManager.
//!
//! Provides aggregation and reporting over tracked process statistics.

use std::collections::HashMap;

use chrono::Utc;

use super::manager::StatsManager;
use super::types::{AppStats, HealthStatus, ProcessStatus};

impl StatsManager {
    /// Get stats for all apps.
    pub fn get_all_stats(&self) -> Vec<AppStats> {
        let mut apps: HashMap<String, AppStats> = HashMap::new();

        for stats in self.stats.values() {
            let app_stats = apps.entry(stats.app.clone()).or_insert_with(|| AppStats {
                app: stats.app.clone(),
                total_processes: 0,
                running_processes: 0,
                healthy_processes: 0,
                total_restarts: 0,
                total_memory_bytes: 0,
                total_cpu_time_ms: 0,
                processes: Vec::new(),
                last_updated: Utc::now(),
            });

            app_stats.total_processes += 1;
            if stats.status == ProcessStatus::Running {
                app_stats.running_processes += 1;
            }
            if stats.health_check_status == HealthStatus::Healthy {
                app_stats.healthy_processes += 1;
            }
            app_stats.total_restarts += stats.restart_count;
            app_stats.total_memory_bytes += stats.memory_bytes;
            app_stats.total_cpu_time_ms += stats.cpu_time_ms;
            app_stats.processes.push(stats.clone());
        }

        apps.into_values().collect()
    }

    /// Remove stats for a process.
    pub fn remove_process(&mut self, process_id: &str) {
        self.stats.remove(process_id);
        self.start_times.remove(process_id);
    }

    /// Remove stats for every process belonging to `app`. For use when the
    /// app itself is gone (e.g. `riku destroy`), as opposed to merely
    /// stopped: a stopped app's stats are kept on purpose so the UI still
    /// shows a `[STOPPED]` row until the next deploy/restart.
    pub fn remove_app(&mut self, app: &str) {
        let process_ids: Vec<String> = self
            .stats
            .values()
            .filter(|s| s.app == app)
            .map(|s| s.process_id.clone())
            .collect();
        for process_id in process_ids {
            self.remove_process(&process_id);
        }
    }
}
