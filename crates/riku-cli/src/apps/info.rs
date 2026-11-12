use anyhow::{bail, Result};
use colored::Colorize;
use riku_supervisor::config::app_worker_configs;
use std::fs;
use std::process::Command;

use crate::config::router::Router;
use crate::config::RikuPaths;
use crate::util::display;
use crate::util::fs::{presence, Presence};

/// `│  label   value`: one detail line with the label padded to 20 columns,
/// so the values of consecutive fields line up.
pub(crate) fn field(label: &str, value: &str) {
    display::detail(&format!("{label:<20} {value}"));
}

/// Print where the host router keeps this app's config.
fn print_route(paths: &RikuPaths, app: &str) {
    match Router::load(paths) {
        Ok(Router::Nginx) => {
            let site = crate::nginx::site_path(app);
            match presence(&site) {
                Presence::Present => field("Nginx Config:", &site.display().to_string()),
                Presence::Missing => {}
                Presence::Unknown => field(
                    "Nginx Config:",
                    &format!(
                        "unknown, {} is not readable by this user",
                        crate::config::NGINX_SITE_ROOT
                    ),
                ),
            }
        }
        Ok(Router::Caddy) => {
            let site = crate::caddy::site_path(app, paths);
            match presence(&site) {
                Presence::Present => field("Caddy Site:", &site.display().to_string()),
                Presence::Missing => {}
                Presence::Unknown => field(
                    "Caddy Site:",
                    &format!(
                        "unknown, {} is not readable by this user",
                        paths.caddy_root.display()
                    ),
                ),
            }
        }
        Ok(Router::Plugin(name)) => field("Router:", &format!("plugin {name}")),
        Err(e) => field("Router:", &format!("unreadable: {e}")),
    }
}

/// Show detailed information about an application.
#[allow(clippy::question_mark)]
pub fn cmd_apps_info(paths: &RikuPaths, app: &str) -> Result<()> {
    let app = match crate::util::validate_app_name(app) {
        Ok(app) => app,
        Err(e) => return Err(e),
    };
    let app_dir = paths.app_root.join(&app);

    if !app_dir.exists() {
        display::error(&format!("app '{}' not found.", app));
        bail!("App not found");
    }

    display::step_done(&format!("App: '{}'", app), "");
    display::detail("");

    field("App Directory:", &app_dir.display().to_string());

    let git_dir = paths.git_root.join(format!("{}.git", app));
    if git_dir.exists() {
        field("Git Repository:", &git_dir.display().to_string());
        field("Git Remote:", &format!("riku@<host>:{app}"));
    }

    print_disk_usage(&app_dir);
    if let Err(e) = print_env_summary(paths, &app) {
        return Err(e);
    }
    if let Err(e) = print_runtime_version(paths, &app) {
        return Err(e);
    }
    if let Err(e) = print_scaling(paths, &app) {
        return Err(e);
    }
    if let Err(e) = print_process_status(paths, &app) {
        return Err(e);
    }
    print_route(paths, &app);

    let log_dir = paths.log_root.join(&app);
    if log_dir.exists() {
        field("Log Directory:", &log_dir.display().to_string());
    }

    let data_dir = paths.data_root.join(&app);
    if data_dir.exists() {
        field("Data Directory:", &data_dir.display().to_string());
    }

    Ok(())
}

/// Print the disk usage of the app directory using `du -sh`.
fn print_disk_usage(app_dir: &std::path::Path) {
    if let Some(app_dir_str) = app_dir.to_str() {
        if let Ok(output) = Command::new("du").args(["-sh", app_dir_str]).output() {
            if let Ok(du_output) = String::from_utf8(output.stdout) {
                if let Some(size) = du_output.split_whitespace().next() {
                    field("Disk Usage:", size);
                }
            }
        }
    }
}

/// Print a summary of the app's merged config (riku.toml, ENV, server
/// override): variable count and key highlights.
#[allow(clippy::question_mark)]
fn print_env_summary(paths: &RikuPaths, app: &str) -> Result<()> {
    let env = match crate::config::app_config::load_env(paths, app) {
        Ok(loaded) => loaded.env,
        Err(e) => return Err(e),
    };
    if env.is_empty() {
        return Ok(());
    }
    field("Env Variables:", &format!("{} configured", env.len()));
    for key in ["RIKU_DOMAINS", "PORT"] {
        if let Some(val) = env.get(key) {
            field(&format!("  {}:", key), val);
        }
    }
    Ok(())
}

/// The runtime version the deploy hands the runtime plugin: only riku.toml
/// `[runtime] version`, read the way the deploy reads it. A
/// `RIKU_<NAME>_VERSION` ENV line is a plain app variable, so it is not
/// shown here.
#[allow(clippy::question_mark)]
fn print_runtime_version(paths: &RikuPaths, app: &str) -> Result<()> {
    let versions = match crate::config::app_config::runtime_versions(paths, app) {
        Ok(versions) => versions,
        Err(e) => return Err(e),
    };
    let mut versions: Vec<_> = versions.into_iter().collect();
    versions.sort();
    for (key, version) in versions {
        field(
            "Runtime Version:",
            &format!("{version} ({key} from riku.toml)"),
        );
    }
    Ok(())
}

/// Print scaling configuration from the SCALING file, if present.
fn print_scaling(paths: &RikuPaths, app: &str) -> Result<()> {
    let scaling_file = paths.env_root.join(app).join("SCALING");
    if scaling_file.exists() {
        let content = match fs::read_to_string(&scaling_file) {
            Ok(content) => content,
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "cannot read {}: {e}",
                    scaling_file.display()
                ))
            }
        };
        let mut scales: Vec<String> = Vec::new();
        for line in content.lines() {
            let line = line.trim();
            if !line.is_empty() && !line.starts_with('#') {
                scales.push(line.to_string());
            }
        }
        if !scales.is_empty() {
            field("Scaling:", &scales.join(", "));
        }
    }
    Ok(())
}

/// Print process status: running/stopped and active worker count.
fn print_process_status(paths: &RikuPaths, app: &str) -> Result<()> {
    let worker_count = app_worker_configs(&paths.workers_enabled, app).len();

    if worker_count > 0 {
        field("Status:", &"running".green().to_string());
        field("Workers:", &format!("{} active", worker_count));
        print_supervisor_stats(paths, app);
    } else {
        field("Status:", &"stopped".yellow().to_string());
    }
    Ok(())
}

/// Print memory and process stats from the supervisor's `stats.json`, if available.
fn print_supervisor_stats(paths: &RikuPaths, app: &str) {
    let stats_file = paths.riku_root.join("stats.json");
    let Some(app_stats_vec) = riku_supervisor::stats::read_stats_file(&stats_file) else {
        return;
    };

    let Some(app_stats) = app_stats_vec.into_iter().find(|s| s.app == app) else {
        return;
    };

    field(
        "Memory:",
        &format!(
            "{:.2} MB",
            app_stats.total_memory_bytes as f64 / 1024.0 / 1024.0
        ),
    );
    field(
        "Running Processes:",
        &app_stats.running_processes.to_string(),
    );
    field(
        "Healthy Processes:",
        &app_stats.healthy_processes.to_string(),
    );
}
