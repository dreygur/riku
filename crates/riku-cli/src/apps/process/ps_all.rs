use anyhow::Result;
use colored::Colorize;
use std::fs;

use riku_supervisor::config::WorkerConfig;
use riku_supervisor::stats::AppStats;

use crate::config::RikuPaths;
use crate::util::display;

/// Show all processes for all apps.
pub fn cmd_ps_all(paths: &RikuPaths, verbose: bool) -> Result<()> {
    let app_root = &paths.app_root;

    if !app_root.exists() {
        display::warn("No applications deployed.");
        return Ok(());
    }

    let mut apps: Vec<String> = fs::read_dir(app_root)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();

    if apps.is_empty() {
        display::warn("No applications deployed.");
        return Ok(());
    }

    apps.sort();

    if verbose {
        show_all_verbose(paths, &apps)
    } else {
        show_all_compact(paths, &apps)
    }
}

fn show_all_verbose(paths: &RikuPaths, apps: &[String]) -> Result<()> {
    let headers = vec!["APP", "PROCESS", "KIND", "PID", "STATUS", "HEALTH"];
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut total_processes = 0;

    let stats_data = load_stats(paths);

    for app in apps {
        let worker_configs = collect_worker_configs(paths, app);

        for config_path in worker_configs {
            let worker_config = match read_worker_config(&config_path) {
                Some(config) => config,
                None => continue,
            };

            let process_name = format!(
                "{}-{}-{}",
                worker_config.worker.app, worker_config.worker.kind, worker_config.worker.ordinal
            );
            let (pid, status, health) = lookup_process_stats(&stats_data, &process_name);

            rows.push(vec![
                app.clone(),
                process_name,
                worker_config.worker.kind,
                pid,
                status,
                health,
            ]);
            total_processes += 1;
        }
    }

    display::section("All Processes");
    crate::util::print_table(&headers, &rows, 2);

    println!(
        "Total: {} process(es) across {} app(s)",
        total_processes.to_string().green(),
        apps.len().to_string().green()
    );
    Ok(())
}

fn show_all_compact(paths: &RikuPaths, apps: &[String]) -> Result<()> {
    let headers = vec!["APP", "WORKERS"];
    let mut rows: Vec<Vec<String>> = Vec::new();

    for app in apps {
        let worker_count = count_worker_configs(paths, app);
        let prefix = if worker_count > 0 { "*" } else { " " };
        rows.push(vec![
            format!("{}{}", prefix, app),
            format!("{} worker(s)", worker_count),
        ]);
    }

    display::section("Deployed Apps");
    crate::util::print_table(&headers, &rows, 2);

    display::blank();
    display::warn("Use 'riku ps <app> --verbose' for detailed process info");
    Ok(())
}

/// Load stats JSON from supervisor stats file, if present.
pub(super) fn load_stats(paths: &RikuPaths) -> Option<Vec<AppStats>> {
    let stats_file = paths.riku_root.join("stats.json");
    riku_supervisor::stats::read_stats_file(&stats_file)
}

/// Look up PID, status, and health for a process from the stats vec.
pub(super) fn lookup_process_stats(
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

/// Parse a worker config TOML file. Configs ending in `.ini` (a legacy
/// extension `collect_worker_configs` still globs for) aren't valid TOML
/// and are skipped here the same as any other unparsable file.
pub(super) fn read_worker_config(path: &std::path::Path) -> Option<WorkerConfig> {
    let content = fs::read_to_string(path).ok()?;
    toml::from_str(&content).ok()
}

/// Collect all worker config paths (toml + ini) for an app.
pub(super) fn collect_worker_configs(paths: &RikuPaths, app: &str) -> Vec<std::path::PathBuf> {
    let toml_pattern = paths.workers_enabled.join(format!("{}-*.toml", app));
    let ini_pattern = paths.workers_enabled.join(format!("{}-*.ini", app));

    let mut configs: Vec<_> = match glob::glob(toml_pattern.to_str().unwrap_or("")) {
        Ok(g) => g.filter_map(|r| r.ok()).collect(),
        Err(e) => {
            display::warn(&format!(
                "Warning: glob failed for toml worker configs: {}",
                e
            ));
            Vec::new()
        }
    };

    let ini_configs: Vec<_> = match glob::glob(ini_pattern.to_str().unwrap_or("")) {
        Ok(g) => g.filter_map(|r| r.ok()).collect(),
        Err(e) => {
            display::warn(&format!(
                "Warning: glob failed for ini worker configs: {}",
                e
            ));
            Vec::new()
        }
    };
    configs.extend(ini_configs);
    configs
}

/// Count total worker configs for an app.
pub(super) fn count_worker_configs(paths: &RikuPaths, app: &str) -> usize {
    let toml_pattern = paths.workers_enabled.join(format!("{}-*.toml", app));
    let ini_pattern = paths.workers_enabled.join(format!("{}-*.ini", app));

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
