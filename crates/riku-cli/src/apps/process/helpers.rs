//! Helper functions for process stats lookup and worker configuration files.

use std::fs;
use std::path::Path;

use riku_supervisor::config::WorkerConfig;
use riku_supervisor::stats::AppStats;

use crate::config::RikuPaths;

/// Load stats JSON from supervisor stats file, if present.
pub(crate) fn load_stats(paths: &RikuPaths) -> Option<Vec<AppStats>> {
    let stats_file = paths.riku_root.join("stats.json");
    riku_supervisor::stats::read_stats_file(&stats_file)
}

/// Look up PID, status, and health for a process from the stats vec.
pub(crate) fn lookup_process_stats(
    stats_data: &Option<Vec<AppStats>>,
    process_name: &str,
) -> (String, String, String) {
    let Some(stats_vec) = stats_data else {
        return (
            "N/A".to_string(),
            "running".to_string(),
            "unknown".to_string(),
        );
    };

    for app_stats in stats_vec {
        if let Some(proc_stats) = app_stats
            .processes
            .iter()
            .find(|p| p.process_id == process_name)
        {
            let pid = proc_stats
                .pid
                .map(|p| p.to_string())
                .unwrap_or_else(|| "N/A".to_string());
            return (
                pid,
                proc_stats.status.to_string(),
                proc_stats.health_check_status.to_string(),
            );
        }
    }

    (
        "N/A".to_string(),
        "unknown".to_string(),
        "unknown".to_string(),
    )
}

/// Parse a worker config TOML file.
pub(crate) fn read_worker_config(path: &Path) -> Option<WorkerConfig> {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return None,
    };
    toml::from_str(&content).ok()
}
