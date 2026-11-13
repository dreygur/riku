use anyhow::Result;
use colored::Colorize;
use riku_supervisor::config::app_worker_configs;
use std::fs;

use super::info::field;
use crate::config::RikuPaths;
use crate::util::{display, require_app};

/// Show stats for all apps.
pub fn cmd_stats_all(paths: &RikuPaths) -> Result<()> {
    let stats_file = paths.riku_root.join("stats.json");

    if let Some(app_stats_vec) = riku_supervisor::stats::read_stats_file(&stats_file) {
        display::step_done("Riku Stats", "");
        display::detail("");

        for app_stats in app_stats_vec {
            let memory_mb = app_stats.total_memory_bytes as f64 / 1024.0 / 1024.0;

            display::step_done(&format!("App: {}", app_stats.app), "");
            field(
                "Processes:",
                &format!(
                    "{}/{} running",
                    app_stats.running_processes, app_stats.total_processes
                ),
            );
            field(
                "Healthy:",
                &format!(
                    "{}/{}",
                    app_stats.healthy_processes, app_stats.total_processes
                ),
            );
            field("Memory:", &format!("{:.2} MB", memory_mb));
            display::detail("");
        }
        return Ok(());
    }

    // Fallback: show basic info from worker configs
    display::step_done("Deployed Apps", "");
    display::detail("");

    if !paths.app_root.exists() {
        display::warn("No apps deployed.");
        return Ok(());
    }

    let entries = match fs::read_dir(&paths.app_root) {
        Ok(entries) => entries,
        Err(e) => {
            return Err(anyhow::anyhow!(
                "cannot read {}: {e}",
                paths.app_root.display()
            ))
        }
    };
    for entry in entries.flatten() {
        let app_name = entry.file_name().to_string_lossy().to_string();

        // Count workers
        let worker_count = app_worker_configs(&paths.workers_enabled, &app_name).len();

        display::detail(&format!("{}: {} workers", app_name.green(), worker_count));
    }

    display::detail("");
    display::detail("Note: Detailed stats require supervisor to be running.");

    Ok(())
}

/// Show stats for a specific app.
#[allow(clippy::question_mark)]
pub fn cmd_stats_app(paths: &RikuPaths, app: &str) -> Result<()> {
    let app = match require_app(app, &paths.app_root) {
        Ok(app) => app,
        Err(e) => return Err(e),
    };

    let stats_file = paths.riku_root.join("stats.json");

    if let Some(app_stats_vec) = riku_supervisor::stats::read_stats_file(&stats_file) {
        if let Some(app_stats) = app_stats_vec.into_iter().find(|s| s.app == app) {
            display::step_done(&format!("Stats for '{}'", app), "");
            display::detail("");

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
    display::step_done(&format!("Processes for '{}'", app), "");

    let worker_configs = app_worker_configs(&paths.workers_enabled, &app);

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

        let process_name = worker_config.worker.name().to_string();
        println!(
            "{:<30} {:<10} {:<10}",
            process_name, worker_config.worker.kind, "running"
        );
    }

    Ok(())
}
