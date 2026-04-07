use anyhow::Result;
use colored::Colorize;
use std::fs;

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
            if let Some(filename) = config_path.file_name().and_then(|s| s.to_str()) {
                let stem = filename.trim_end_matches(".toml").trim_end_matches(".ini");
                let prefix = format!("{}-", app);
                let remainder = stem.strip_prefix(prefix.as_str()).unwrap_or("");
                if let Some((kind, ordinal)) = remainder.split_once('-') {
                    let process_name = format!("{}-{}-{}", app, kind, ordinal);
                    let (pid, status, health) = lookup_process_stats(&stats_data, &process_name);

                    rows.push(vec![
                        app.clone(),
                        process_name,
                        kind.to_string(),
                        pid,
                        status,
                        health,
                    ]);
                    total_processes += 1;
                }
            }
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
pub(super) fn load_stats(paths: &RikuPaths) -> Option<Vec<serde_json::Value>> {
    let stats_file = paths.riku_root.join("stats.json");
    if stats_file.exists() {
        fs::read_to_string(&stats_file)
            .ok()
            .and_then(|content| serde_json::from_str::<Vec<serde_json::Value>>(&content).ok())
    } else {
        None
    }
}

/// Look up PID, status, and health for a process from the stats vec.
pub(super) fn lookup_process_stats(
    stats_data: &Option<Vec<serde_json::Value>>,
    process_name: &str,
) -> (String, String, String) {
    if let Some(stats_vec) = stats_data {
        let mut pid = "N/A".to_string();
        let mut status = "unknown".to_string();
        let mut health = "unknown".to_string();

        for app_stats in stats_vec {
            if let Some(processes) = app_stats.get("processes").and_then(|v| v.as_array()) {
                for proc_stats in processes {
                    if let Some(proc_id) = proc_stats.get("process_id").and_then(|v| v.as_str()) {
                        if proc_id == process_name {
                            pid = proc_stats
                                .get("pid")
                                .and_then(|v| v.as_u64())
                                .map(|p| p.to_string())
                                .unwrap_or_else(|| "N/A".to_string());
                            status = proc_stats
                                .get("status")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown")
                                .to_string();
                            health = proc_stats
                                .get("health_check_status")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown")
                                .to_string();
                            break;
                        }
                    }
                }
            }
        }
        (pid, status, health)
    } else {
        (
            "N/A".to_string(),
            "running".to_string(),
            "unknown".to_string(),
        )
    }
}

/// Collect all worker config paths (toml + ini) for an app.
pub(super) fn collect_worker_configs(paths: &RikuPaths, app: &str) -> Vec<std::path::PathBuf> {
    let toml_pattern = paths.workers_enabled.join(format!("{}-*.toml", app));
    let ini_pattern = paths.workers_enabled.join(format!("{}-*.ini", app));

    let mut configs: Vec<_> = match glob::glob(toml_pattern.to_str().unwrap_or("")) {
        Ok(g) => g.filter_map(|r| r.ok()).collect(),
        Err(e) => {
            display::warn(&format!("Warning: glob failed for toml worker configs: {}", e));
            Vec::new()
        }
    };

    let ini_configs: Vec<_> = match glob::glob(ini_pattern.to_str().unwrap_or("")) {
        Ok(g) => g.filter_map(|r| r.ok()).collect(),
        Err(e) => {
            display::warn(&format!("Warning: glob failed for ini worker configs: {}", e));
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
