use anyhow::Result;
use colored::Colorize;
use std::fs;

use crate::config::RikuPaths;
use crate::util::display;

/// List apps, marking running ones with '*'.
pub fn cmd_apps(paths: &RikuPaths) -> Result<()> {
    let app_root = &paths.app_root;
    if !app_root.exists() {
        display::note("There are no applications deployed.");
        display::warn("Deploy your first app:");
        display::warn("  git remote add riku deploy@your-server:myapp");
        display::warn("  git push riku main");
        return Ok(());
    }

    let mut apps: Vec<String> = fs::read_dir(app_root)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();

    if apps.is_empty() {
        display::note("There are no applications deployed.");
        display::warn("Deploy your first app:");
        display::warn("  git remote add riku deploy@your-server:myapp");
        display::warn("  git push riku main");
        return Ok(());
    }

    apps.sort();

    // Build table data
    let headers = vec!["APP", "STATUS", "WORKERS"];
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut total_workers = 0;

    for a in &apps {
        let ini_pattern = paths.workers_enabled.join(format!("{}-*.ini", a));
        let toml_pattern = paths.workers_enabled.join(format!("{}-*.toml", a));
        let ini_matches = glob::glob(ini_pattern.to_str().unwrap_or(""))
            .map(|g| g.count())
            .unwrap_or_else(|e| {
                display::warn(&format!("Warning: glob failed for ini pattern: {}", e));
                0
            });
        let toml_matches = glob::glob(toml_pattern.to_str().unwrap_or(""))
            .map(|g| g.count())
            .unwrap_or_else(|e| {
                display::warn(&format!("Warning: glob failed for toml pattern: {}", e));
                0
            });
        let worker_count = ini_matches + toml_matches;
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
