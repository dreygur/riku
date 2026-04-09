use anyhow::Result;
use colored::Colorize;
use serde_json;
use std::fs;

use crate::config::RikuPaths;
use crate::util::{display, exit_if_invalid};

/// Show stats for all apps.
pub fn cmd_stats_all(paths: &RikuPaths) -> Result<()> {
    // Read stats from the supervisor's stats file if it exists
    let stats_file = paths.riku_root.join("stats.json");

    if stats_file.exists() {
        if let Ok(content) = fs::read_to_string(&stats_file) {
            if let Ok(stats_vec) = serde_json::from_str::<Vec<serde_json::Value>>(&content) {
                display::section("Riku Stats");
                display::blank();

                for stats in stats_vec {
                    if let Some(app) = stats.get("app").and_then(|v| v.as_str()) {
                        let total_procs = stats
                            .get("total_processes")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0);
                        let running_procs = stats
                            .get("running_processes")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0);
                        let healthy_procs = stats
                            .get("healthy_processes")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0);
                        let memory_bytes = stats
                            .get("total_memory_bytes")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0);
                        let memory_mb = memory_bytes as f64 / 1024.0 / 1024.0;

                        display::info(&format!("App: {}", app));
                        display::kv(
                            "Processes:",
                            &format!("{}/{} running", running_procs, total_procs),
                        );
                        display::kv("Healthy:", &format!("{}/{}", healthy_procs, total_procs));
                        display::kv("Memory:", &format!("{:.2} MB", memory_mb));
                        display::blank();
                    }
                }
                return Ok(());
            }
        }
    }

    // Fallback: show basic info from worker configs
    display::section("Deployed Apps");
    display::blank();

    if !paths.app_root.exists() {
        display::warn("No apps deployed.");
        return Ok(());
    }

    for entry in fs::read_dir(&paths.app_root)?.flatten() {
        let app_name = entry.file_name().to_string_lossy().to_string();

        // Count workers
        let toml_pattern = paths.workers_enabled.join(format!("{}-*.toml", app_name));
        let worker_count = glob::glob(toml_pattern.to_str().unwrap_or(""))
            .map(|g| g.count())
            .unwrap_or(0);

        display::note(&format!("{}: {} workers", app_name.green(), worker_count));
    }

    display::blank();
    display::note("Note: Detailed stats require supervisor to be running.");

    Ok(())
}

/// Show stats for a specific app.
pub fn cmd_stats_app(paths: &RikuPaths, app: &str) -> Result<()> {
    let app = exit_if_invalid(app, &paths.app_root)?;

    // Read stats from the supervisor's stats file if it exists
    let stats_file = paths.riku_root.join("stats.json");

    if stats_file.exists() {
        if let Ok(content) = fs::read_to_string(&stats_file) {
            if let Ok(stats_vec) = serde_json::from_str::<Vec<serde_json::Value>>(&content) {
                for stats in stats_vec {
                    if let Some(stats_app) = stats.get("app").and_then(|v| v.as_str()) {
                        if stats_app == app {
                            display::section(&format!("Stats for '{}'", app));
                            display::blank();

                            if let Some(processes) =
                                stats.get("processes").and_then(|v| v.as_array())
                            {
                                println!(
                                    "{:<25} {:<10} {:<10} {:<12} {:<15}",
                                    "PROCESS", "KIND", "PID", "STATUS", "HEALTH"
                                );
                                println!("{}", "-".repeat(75));

                                for proc_stats in processes {
                                    let process_id = proc_stats
                                        .get("process_id")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("unknown");
                                    let kind = proc_stats
                                        .get("kind")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("unknown");
                                    let pid = proc_stats
                                        .get("pid")
                                        .and_then(|v| v.as_u64())
                                        .map(|p| p.to_string())
                                        .unwrap_or_else(|| "N/A".to_string());
                                    let status = proc_stats
                                        .get("status")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("unknown");
                                    let health = proc_stats
                                        .get("health_check_status")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("unknown");

                                    println!(
                                        "{:<25} {:<10} {:<10} {:<12} {:<15}",
                                        process_id, kind, pid, status, health
                                    );
                                }
                            }

                            if let Some(mem) =
                                stats.get("total_memory_bytes").and_then(|v| v.as_u64())
                            {
                                println!();
                                println!("Total Memory: {:.2} MB", mem as f64 / 1024.0 / 1024.0);
                            }

                            return Ok(());
                        }
                    }
                }
            }
        }
    }

    // Fallback: show basic info
    display::section(&format!("Processes for '{}'", app));

    let toml_pattern = paths.workers_enabled.join(format!("{}-*.toml", app));
    let worker_configs: Vec<_> = glob::glob(toml_pattern.to_str().unwrap_or(""))
        .map(|g| g.filter_map(|r| r.ok()).collect())
        .unwrap_or_else(|_| Vec::new());

    if worker_configs.is_empty() {
        display::warn("No running processes found.");
        return Ok(());
    }

    println!("{:<30} {:<10} {:<10}", "PROCESS", "KIND", "STATUS");
    println!("{}", "-".repeat(55));

    for config_path in worker_configs {
        if let Some(filename) = config_path.file_name().and_then(|s| s.to_str()) {
            let parts: Vec<&str> = filename.trim_end_matches(".toml").split('-').collect();
            if parts.len() >= 3 {
                let kind = parts[1];
                let ordinal = parts.get(2).unwrap_or(&"1");
                let process_name = format!("{}-{}-{}", app, kind, ordinal);
                println!("{:<30} {:<10} {:<10}", process_name, kind, "running");
            }
        }
    }

    Ok(())
}
