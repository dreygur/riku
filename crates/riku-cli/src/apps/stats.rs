use anyhow::Result;
use colored::Colorize;
use std::fs;

use crate::config::RikuPaths;
use crate::util::{display, exit_if_invalid};

/// Show stats for all apps.
pub fn cmd_stats_all(paths: &RikuPaths) -> Result<()> {
    let stats_file = paths.riku_root.join("stats.json");

    if let Some(app_stats_vec) = riku_supervisor::stats::read_stats_file(&stats_file) {
        display::section("Riku Stats");
        display::blank();

        for app_stats in app_stats_vec {
            let memory_mb = app_stats.total_memory_bytes as f64 / 1024.0 / 1024.0;

            display::info(&format!("App: {}", app_stats.app));
            display::kv(
                "Processes:",
                &format!(
                    "{}/{} running",
                    app_stats.running_processes, app_stats.total_processes
                ),
            );
            display::kv(
                "Healthy:",
                &format!(
                    "{}/{}",
                    app_stats.healthy_processes, app_stats.total_processes
                ),
            );
            display::kv("Memory:", &format!("{:.2} MB", memory_mb));
            display::blank();
        }
        return Ok(());
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

    let stats_file = paths.riku_root.join("stats.json");

    if let Some(app_stats_vec) = riku_supervisor::stats::read_stats_file(&stats_file) {
        if let Some(app_stats) = app_stats_vec.into_iter().find(|s| s.app == app) {
            display::section(&format!("Stats for '{}'", app));
            display::blank();

            println!(
                "{:<25} {:<10} {:<10} {:<12} {:<15}",
                "PROCESS", "KIND", "PID", "STATUS", "HEALTH"
            );
            println!("{}", "-".repeat(75));

            for proc_stats in &app_stats.processes {
                let pid = proc_stats
                    .pid
                    .map(|p| p.to_string())
                    .unwrap_or_else(|| "N/A".to_string());

                println!(
                    "{:<25} {:<10} {:<10} {:<12} {:<15}",
                    proc_stats.process_id,
                    proc_stats.kind,
                    pid,
                    proc_stats.status,
                    proc_stats.health_check_status,
                );
            }

            println!();
            println!(
                "Total Memory: {:.2} MB",
                app_stats.total_memory_bytes as f64 / 1024.0 / 1024.0
            );

            return Ok(());
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
        let content = match fs::read_to_string(&config_path) {
            Ok(content) => content,
            Err(_) => continue,
        };
        let worker_config: riku_supervisor::config::WorkerConfig = match toml::from_str(&content) {
            Ok(config) => config,
            Err(_) => continue,
        };

        let process_name = format!(
            "{}-{}-{}",
            worker_config.worker.app, worker_config.worker.kind, worker_config.worker.ordinal
        );
        println!(
            "{:<30} {:<10} {:<10}",
            process_name, worker_config.worker.kind, "running"
        );
    }

    Ok(())
}
