use anyhow::Result;
use colored::Colorize;
use riku_supervisor::config::app_worker_configs;
use std::fs;

use crate::config::RikuPaths;
use crate::util::display;

/// How to push an app from the laptop. The server cannot know the name the
/// laptop reaches it by, so `<host>` stays for the developer to fill in.
pub(super) fn print_first_deploy_hint() {
    display::warn("From your app's git repo on your laptop:");
    display::warn("  rk server add <name> riku@<host> --admin <login>@<host>");
    display::warn("  git push riku main");
}

/// List apps, marking running ones with '*'.
pub fn cmd_apps(paths: &RikuPaths) -> Result<()> {
    let app_root = &paths.app_root;
    if !app_root.exists() {
        display::note("There are no applications deployed.");
        print_first_deploy_hint();
        return Ok(());
    }

    let mut apps: Vec<String> = fs::read_dir(app_root)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();

    if apps.is_empty() {
        display::note("There are no applications deployed.");
        print_first_deploy_hint();
        return Ok(());
    }

    apps.sort();

    // Build table data
    let headers = vec!["APP", "STATUS", "WORKERS"];
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut total_workers = 0;

    for a in &apps {
        let worker_count = app_worker_configs(&paths.workers_enabled, a).len();
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

    display::section("Deployed Apps");
    crate::util::print_table(&headers, &rows, 2);

    display::blank();
    println!(
        "Total: {} app(s), {} worker(s) running",
        apps.len().to_string().green(),
        total_workers.to_string().green()
    );
    display::blank();
    display::warn("* = running");

    Ok(())
}
