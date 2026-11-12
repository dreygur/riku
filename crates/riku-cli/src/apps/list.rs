use anyhow::Result;
use colored::Colorize;
use riku_supervisor::config::app_worker_configs;
use riku_supervisor::stats::{AppStats, ProcessStatus};
use std::fs;

use super::process::helpers::load_stats;
use crate::config::RikuPaths;
use crate::util::display;

/// How many of `app`'s containers the supervisor last saw running. A
/// container app has no worker configs, so this is what makes it count as
/// running.
fn running_containers(stats: Option<&[AppStats]>, app: &str) -> usize {
    stats
        .unwrap_or_default()
        .iter()
        .filter(|entry| entry.app == app)
        .flat_map(|entry| &entry.processes)
        .filter(|p| p.container.is_some() && p.status == ProcessStatus::Running)
        .count()
}

/// How to push an app from the laptop. The server cannot know the name the
/// laptop reaches it by, so `<host>` stays for the developer to fill in.
pub(super) fn print_first_deploy_hint() {
    display::warn("From your app's git repo on your laptop:");
    display::detail_err("rk server add <name> riku@<host> --admin <login>@<host>");
    display::detail_err("git push riku main");
}

/// List apps, marking running ones with '*'.
pub fn cmd_apps(paths: &RikuPaths) -> Result<()> {
    let app_root = &paths.app_root;
    if !app_root.exists() {
        display::detail("There are no applications deployed.");
        print_first_deploy_hint();
        return Ok(());
    }

    let entries = match fs::read_dir(app_root) {
        Ok(entries) => entries,
        Err(e) => return Err(anyhow::anyhow!("cannot read {}: {e}", app_root.display())),
    };
    let mut apps: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();

    if apps.is_empty() {
        display::detail("There are no applications deployed.");
        print_first_deploy_hint();
        return Ok(());
    }

    apps.sort();

    // Build table data
    let headers = vec!["APP", "STATUS", "WORKERS"];
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut total_workers = 0;

    let stats = load_stats(paths);
    for a in &apps {
        let worker_count = app_worker_configs(&paths.workers_enabled, a).len()
            + running_containers(stats.as_deref(), a);
        let status = if worker_count > 0 {
            "running"
        } else {
            "stopped"
        };
        let prefix = if worker_count > 0 { "*" } else { " " };

        rows.push(vec![
            format!("{}{}", prefix, a),
            status.to_string(),
            worker_count.to_string(),
        ]);

        total_workers += worker_count;
    }

    display::step_done("Deployed Apps", "");
    crate::util::print_table(&headers, &rows, 2);

    display::detail("");
    println!(
        "Total: {} app(s), {} worker(s) running",
        apps.len().to_string().green(),
        total_workers.to_string().green()
    );
    display::detail("");
    display::warn("* = running");

    Ok(())
}

#[cfg(test)]
#[path = "list_tests.rs"]
mod tests;
