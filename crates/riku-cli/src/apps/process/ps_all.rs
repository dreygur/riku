use std::fs;

use anyhow::Result;
use colored::Colorize;
use riku_supervisor::config::app_worker_configs;

use crate::config::RikuPaths;
use crate::util::display;

/// Show all processes for all apps.
#[allow(clippy::question_mark)]
pub fn cmd_ps_all(paths: &RikuPaths, verbose: bool) -> Result<()> {
    let app_root = &paths.app_root;

    if !app_root.exists() {
        display::warn("No applications deployed.");
        return Ok(());
    }

    let entries = match fs::read_dir(app_root) {
        Ok(e) => e,
        Err(e) => return Err(e.into()),
    };

    let mut apps: Vec<String> = entries
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
        let worker_configs = app_worker_configs(&paths.workers_enabled, app);

        for config_path in worker_configs {
            let worker_config = match read_worker_config(&config_path) {
                Some(config) => config,
                None => continue,
            };

            let process_name = worker_config.worker.name().to_string();
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
        let worker_count = app_worker_configs(&paths.workers_enabled, app).len();
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

pub(super) use super::helpers::{load_stats, lookup_process_stats, read_worker_config};
