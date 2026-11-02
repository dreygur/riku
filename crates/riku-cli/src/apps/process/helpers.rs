//! Helper functions for process stats lookup and worker configuration files.

use std::fs;
use std::path::{Path, PathBuf};

use riku_supervisor::config::WorkerConfig;
use riku_supervisor::stats::AppStats;

use crate::config::RikuPaths;
use crate::util::display;

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

/// Collect all worker config paths (toml + ini) for an app.
pub(crate) fn collect_worker_configs(paths: &RikuPaths, app: &str) -> Vec<PathBuf> {
    let toml_pattern = paths.workers_enabled.join(format!("{app}-*.toml"));
    let ini_pattern = paths.workers_enabled.join(format!("{app}-*.ini"));

    let mut configs: Vec<PathBuf> = match glob::glob(toml_pattern.to_str().unwrap_or("")) {
        Ok(g) => g.filter_map(Result::ok).collect(),
        Err(e) => {
            display::warn(&format!(
                "Warning: glob failed for toml worker configs: {e}"
            ));
            Vec::new()
        }
    };

    let ini_configs: Vec<PathBuf> = match glob::glob(ini_pattern.to_str().unwrap_or("")) {
        Ok(g) => g.filter_map(Result::ok).collect(),
        Err(e) => {
            display::warn(&format!("Warning: glob failed for ini worker configs: {e}"));
            Vec::new()
        }
    };
    configs.extend(ini_configs);
    configs
}

/// Count total worker configs for an app.
pub(crate) fn count_worker_configs(paths: &RikuPaths, app: &str) -> usize {
    let toml_pattern = paths.workers_enabled.join(format!("{app}-*.toml"));
    let ini_pattern = paths.workers_enabled.join(format!("{app}-*.ini"));

    let toml_count = match glob::glob(toml_pattern.to_str().unwrap_or("")) {
        Ok(g) => g.count(),
        Err(_) => 0,
    };
    let ini_count = match glob::glob(ini_pattern.to_str().unwrap_or("")) {
        Ok(g) => g.count(),
        Err(_) => 0,
    };
    toml_count + ini_count
}
