use anyhow::{bail, Result};
use colored::Colorize;
use std::collections::HashMap;
use std::fs;
use std::process::Command;

use crate::config::RikuPaths;
use crate::util::{echo, parse_settings};

/// Show detailed information about an application.
pub fn cmd_apps_info(paths: &RikuPaths, app: &str) -> Result<()> {
    let app = crate::util::validate_app_name(app)?;
    let app_dir = paths.app_root.join(&app);

    if !app_dir.exists() {
        echo(&format!("Error: app '{}' not found.", app), "red");
        bail!("App not found");
    }

    println!("{}", format!("=== App: '{}' ===", app).green());
    println!();

    println!("App Directory: {}", app_dir.display());

    let git_dir = paths.git_root.join(format!("{}.git", app));
    if git_dir.exists() {
        println!("Git Repository: {}", git_dir.display());
        println!("Git Remote: deploy@your-server:{}", app);
    }

    print_disk_usage(&app_dir);
    print_env_summary(paths, &app)?;
    print_scaling(paths, &app)?;
    print_process_status(paths, &app)?;

    let nginx_conf = paths.nginx_root.join(format!("{}.conf", app));
    if nginx_conf.exists() {
        println!("Nginx Config: {}", nginx_conf.display());
    }

    let log_dir = paths.log_root.join(&app);
    if log_dir.exists() {
        println!("Log Directory: {}", log_dir.display());
    }

    let data_dir = paths.data_root.join(&app);
    if data_dir.exists() {
        println!("Data Directory: {}", data_dir.display());
    }

    Ok(())
}

fn print_disk_usage(app_dir: &std::path::Path) {
    if let Some(app_dir_str) = app_dir.to_str() {
        if let Ok(output) = Command::new("du").args(["-sh", app_dir_str]).output() {
            if let Ok(du_output) = String::from_utf8(output.stdout) {
                if let Some(size) = du_output.split_whitespace().next() {
                    println!("Disk Usage: {}", size);
                }
            }
        }
    }
}

fn print_env_summary(paths: &RikuPaths, app: &str) -> Result<()> {
    let env_file = paths.env_root.join(app).join("ENV");
    if env_file.exists() {
        let mut env = HashMap::new();
        parse_settings(&env_file, &mut env)?;
        let var_count = env.len();
        println!("Environment Variables: {} configured", var_count);

        for key in ["NGINX_SERVER_NAME", "NODE_VERSION", "PORT"] {
            if let Some(val) = env.get(key) {
                println!("  {}: {}", key, val);
            }
        }
    }
    Ok(())
}

fn print_scaling(paths: &RikuPaths, app: &str) -> Result<()> {
    let scaling_file = paths.env_root.join(app).join("SCALING");
    if scaling_file.exists() {
        let content = fs::read_to_string(&scaling_file)?;
        let mut scales: Vec<String> = Vec::new();
        for line in content.lines() {
            let line = line.trim();
            if !line.is_empty() && !line.starts_with('#') {
                scales.push(line.to_string());
            }
        }
        if !scales.is_empty() {
            println!("Scaling: {}", scales.join(", "));
        }
    }
    Ok(())
}

fn print_process_status(paths: &RikuPaths, app: &str) -> Result<()> {
    let toml_pattern = paths.workers_enabled.join(format!("{}-*.toml", app));
    let ini_pattern = paths.workers_enabled.join(format!("{}-*.ini", app));

    let toml_count = glob::glob(toml_pattern.to_str().unwrap_or(""))
        .map(|g| g.count())
        .unwrap_or(0);
    let ini_count = glob::glob(ini_pattern.to_str().unwrap_or(""))
        .map(|g| g.count())
        .unwrap_or(0);
    let worker_count = toml_count + ini_count;

    if worker_count > 0 {
        println!("Status: {} running", "running".green());
        println!("Workers: {} active", worker_count);
        print_supervisor_stats(paths, app);
    } else {
        println!("Status: {}", "stopped".yellow());
    }
    Ok(())
}

fn print_supervisor_stats(paths: &RikuPaths, app: &str) {
    let stats_file = paths.riku_root.join("stats.json");
    if stats_file.exists() {
        if let Ok(content) = fs::read_to_string(&stats_file) {
            if let Ok(stats_vec) = serde_json::from_str::<Vec<serde_json::Value>>(&content) {
                for app_stats in stats_vec {
                    if let Some(app_name) = app_stats.get("app").and_then(|v| v.as_str()) {
                        if app_name == app {
                            if let Some(mem) =
                                app_stats.get("total_memory_bytes").and_then(|v| v.as_u64())
                            {
                                println!("Memory: {:.2} MB", mem as f64 / 1024.0 / 1024.0);
                            }
                            if let Some(running) =
                                app_stats.get("running_processes").and_then(|v| v.as_u64())
                            {
                                println!("Running Processes: {}", running);
                            }
                            if let Some(healthy) =
                                app_stats.get("healthy_processes").and_then(|v| v.as_u64())
                            {
                                println!("Healthy Processes: {}", healthy);
                            }
                            break;
                        }
                    }
                }
            }
        }
    }
}
