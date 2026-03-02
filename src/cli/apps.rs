use anyhow::{bail, Result};
use colored::Colorize;
use serde_json;
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use crate::config::{RikuPaths, RIKU_RAW_SOURCE_URL};
use crate::supervisor::Supervisor;
use crate::util::{
    echo, ensure_path_within, exit_if_invalid, parse_settings, sanitize_app_name, write_config,
};

/// List apps, marking running ones with '*'.
pub fn cmd_apps(paths: &RikuPaths) -> Result<()> {
    let app_root = &paths.app_root;
    if !app_root.exists() {
        echo("There are no applications deployed.", "");
        echo("Deploy your first app:", "yellow");
        echo("  git remote add riku deploy@your-server:myapp", "yellow");
        echo("  git push riku main", "yellow");
        return Ok(());
    }

    let mut apps: Vec<String> = fs::read_dir(app_root)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();

    if apps.is_empty() {
        echo("There are no applications deployed.", "");
        echo("Deploy your first app:", "yellow");
        echo("  git remote add riku deploy@your-server:myapp", "yellow");
        echo("  git push riku main", "yellow");
        return Ok(());
    }

    apps.sort();

    // Build table data
    let headers = vec!["APP", "STATUS", "WORKERS"];
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut total_workers = 0;

    for a in &apps {
        // Check for running worker configs
        let ini_pattern = paths.workers_enabled.join(format!("{}*.ini", a));
        let toml_pattern = paths.workers_enabled.join(format!("{}*.toml", a));
        let ini_matches = glob::glob(ini_pattern.to_str().unwrap_or(""))
            .map(|g| g.count())
            .unwrap_or_else(|e| {
                eprintln!("Warning: glob failed for ini pattern: {}", e);
                0
            });
        let toml_matches = glob::glob(toml_pattern.to_str().unwrap_or(""))
            .map(|g| g.count())
            .unwrap_or_else(|e| {
                eprintln!("Warning: glob failed for toml pattern: {}", e);
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

    // Print table using utility
    crate::util::print_table_with_title("=== Deployed Apps ===", &headers, &rows, 2);

    println!();
    println!(
        "Total: {} app(s), {} worker(s) running",
        apps.len().to_string().green(),
        total_workers.to_string().green()
    );
    println!();
    echo("* = running", "yellow");

    Ok(())
}

/// Show app configuration (ENV file).
pub fn cmd_config_show(paths: &RikuPaths, app: &str) -> Result<()> {
    let app = exit_if_invalid(app, &paths.app_root)?;

    let config_file = paths.env_root.join(&app).join("ENV");
    if config_file.exists() {
        let content = fs::read_to_string(&config_file)?;
        println!("{}", content.trim().white());
    } else {
        echo(
            &format!("Warning: app '{}' not deployed, no config found.", app),
            "yellow",
        );
    }
    Ok(())
}

/// Get a single config value.
pub fn cmd_config_get(paths: &RikuPaths, app: &str, key: &str) -> Result<()> {
    let app = exit_if_invalid(app, &paths.app_root)?;

    let config_file = paths.env_root.join(&app).join("ENV");
    if config_file.exists() {
        let mut env = HashMap::new();
        let settings = parse_settings(&config_file, &mut env)?;
        if let Some(val) = settings.get(key) {
            println!("{}", val.white());
        }
    } else {
        echo(
            &format!("Warning: no active configuration for '{}'", app),
            "",
        );
    }
    Ok(())
}

/// Set config values (KEY=VALUE pairs), write config, trigger deploy.
pub fn cmd_config_set(paths: &RikuPaths, app: &str, settings: &[String]) -> Result<()> {
    let app = exit_if_invalid(app, &paths.app_root)?;

    let config_file = paths.env_root.join(&app).join("ENV");
    let mut env = HashMap::new();
    parse_settings(&config_file, &mut env)?;

    // Join all settings and split them shell-style
    let joined = settings.join(" ");
    let parts = shell_split(&joined);

    for s in &parts {
        if let Some(eq_pos) = s.find('=') {
            let k = s[..eq_pos].trim().to_string();
            let v = s[eq_pos + 1..].trim().to_string();
            println!("{}", format!("Setting {}={} for '{}'", k, v, app).white());
            env.insert(k, v);
        } else {
            echo(&format!("Error: malformed setting '{}'", s), "red");
            return Ok(());
        }
    }
    write_config(&config_file, &env, "=")?;
    // Trigger a deploy after config change
    let deltas: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    crate::deploy::do_deploy(&app, paths, &deltas, None)?;
    Ok(())
}

/// Unset config values, write config, trigger deploy.
pub fn cmd_config_unset(paths: &RikuPaths, app: &str, keys: &[String]) -> Result<()> {
    let app = exit_if_invalid(app, &paths.app_root)?;

    let config_file = paths.env_root.join(&app).join("ENV");
    let mut env = HashMap::new();
    parse_settings(&config_file, &mut env)?;

    for s in keys {
        if env.remove(s).is_some() {
            println!("{}", format!("Unsetting {} for '{}'", s, app).white());
        }
    }
    write_config(&config_file, &env, "=")?;
    // Trigger a deploy after config change
    let deltas: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    crate::deploy::do_deploy(&app, paths, &deltas, None)?;
    Ok(())
}

/// Show live running configuration (LIVE_ENV file).
pub fn cmd_config_live(paths: &RikuPaths, app: &str) -> Result<()> {
    let app = exit_if_invalid(app, &paths.app_root)?;

    let live_config = paths.env_root.join(&app).join("LIVE_ENV");
    if live_config.exists() {
        let content = fs::read_to_string(&live_config)?;
        println!("{}", content.trim().white());
    } else {
        echo(
            &format!("Warning: app '{}' not deployed, no config found.", app),
            "yellow",
        );
    }
    Ok(())
}

/// Deploy an app.
pub fn cmd_deploy(paths: &RikuPaths, app: &str, from_path: Option<&str>) -> Result<()> {
    let deltas: std::collections::HashMap<String, i64> = std::collections::HashMap::new();

    // If deploying from local path, copy files first (creates app directory)
    if let Some(source_path) = from_path {
        deploy_from_path(paths, app, source_path)?;
    } else if is_bare_repo() {
        // Deploying from a bare repo - extract files and set up hook
        deploy_from_bare_repo(paths, app)?;
    } else {
        // For git-based deploy, app must already exist
        let _ = exit_if_invalid(app, &paths.app_root)?;
    }

    crate::deploy::do_deploy(app, paths, &deltas, None)
}

/// Check if current directory is a bare git repo.
fn is_bare_repo() -> bool {
    Command::new("git")
        .args(["rev-parse", "--is-bare-repository"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim() == "true")
        .unwrap_or(false)
}

/// Deploy from a bare repo by extracting files and setting up auto-deploy hook.
fn deploy_from_bare_repo(paths: &RikuPaths, app: &str) -> Result<()> {
    // Get current directory (the bare repo)
    let bare_repo = std::env::current_dir()?;

    // Ensure symlink is set up
    crate::cli::git::ensure_repo_symlink(paths, app)?;

    // Extract files from bare repo to app directory
    crate::cli::git::extract_bare_repo_to_app(&bare_repo, app, paths)?;

    // Set up post-receive hook for auto-deploy on push
    crate::cli::git::setup_post_receive_hook(&bare_repo, app)?;

    Ok(())
}

/// Deploy from a local path (copies files to app directory).
fn deploy_from_path(paths: &RikuPaths, app: &str, source: &str) -> Result<()> {
    use std::path::Path;

    let source_path = Path::new(source);

    // Validate source path
    if !source_path.exists() {
        echo(&format!("Error: path '{}' does not exist.", source), "red");
        bail!("Source path does not exist");
    }

    if !source_path.is_dir() {
        echo(&format!("Error: '{}' is not a directory.", source), "red");
        bail!("Source is not a directory");
    }

    // Check for required files
    let procfile = source_path.join("Procfile");
    if !procfile.exists() {
        echo("Error: Procfile not found in source directory.", "red");
        echo("A Procfile is required for deployment.", "yellow");
        echo("Example: echo 'web: npm start' > Procfile", "yellow");
        bail!("Procfile not found");
    }

    // Check if it's a git repo (optional but recommended)
    let git_dir = source_path.join(".git");
    if !git_dir.exists() {
        echo("⚠ Warning: source is not a git repository.", "yellow");
        echo("  Consider initializing git: git init", "yellow");
    }

    // Copy files to app directory
    let app_dir = paths.app_root.join(app);
    echo(&format!("Copying files from '{}'...", source), "green");

    // Remove existing app files (preserve data dir)
    if app_dir.exists() {
        fs::remove_dir_all(&app_dir)?;
    }

    // Copy source to app directory
    copy_dir_recursive(source_path, &app_dir)?;

    echo(
        &format!("✓ Copied {} files", count_files(&app_dir)?),
        "green",
    );

    Ok(())
}

/// Recursively copy a directory.
fn copy_dir_recursive(source: &Path, dest: &Path) -> Result<()> {
    fs::create_dir_all(dest)?;

    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let entry_path = entry.path();
        let dest_path = dest.join(entry.file_name());

        if entry_path.is_dir() {
            // Skip certain directories (git, node_modules - will be installed)
            if entry_path
                .file_name()
                .map(|n| n == ".git" || n == "node_modules" || n == ".gitignore")
                .unwrap_or(false)
            {
                continue;
            }
            copy_dir_recursive(&entry_path, &dest_path)?;
        } else {
            fs::copy(&entry_path, &dest_path)?;
        }
    }

    Ok(())
}

/// Count files in a directory.
fn count_files(dir: &Path) -> Result<usize> {
    let mut count = 0;
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                count += count_files(&path)?;
            } else {
                count += 1;
            }
        }
    }
    Ok(count)
}

/// Destroy an app — remove directories and config files, preserve data/cache.
pub fn cmd_destroy(paths: &RikuPaths, app: &str) -> Result<()> {
    let app = exit_if_invalid(app, &paths.app_root)?;

    // Remove app, git, env, log directories
    for dir in [
        &paths.app_root,
        &paths.git_root,
        &paths.env_root,
        &paths.log_root,
    ] {
        let p = dir.join(&app);
        if p.exists() {
            echo(&format!("--> Removing folder '{}'", p.display()), "yellow");
            fs::remove_dir_all(&p)?;
        }
    }

    // Remove worker config files (*.ini and *.toml)
    for dir in [&paths.workers_available, &paths.workers_enabled] {
        for ext in &["ini", "toml"] {
            let pattern = dir.join(format!("{}*.{}", app, ext));
            if let Ok(entries) = glob::glob(pattern.to_str().unwrap_or("")) {
                for entry in entries.flatten() {
                    echo(
                        &format!("--> Removing file '{}'", entry.display()),
                        "yellow",
                    );
                    fs::remove_file(&entry)?;
                }
            }
        }
    }

    // Remove nginx configuration and associated files
    echo(
        &format!("--> Removing nginx config for '{}'", app),
        "yellow",
    );
    crate::nginx::remove_nginx_config(&app, paths)?;

    // Remove ACME certs if they exist
    let acme_link = paths.acme_www.join(&app);
    if acme_link.exists() {
        let acme_certs = fs::canonicalize(&acme_link).unwrap_or_else(|_| acme_link.clone());
        if acme_certs.exists() {
            // Verify the resolved path doesn't escape the riku directory tree
            if ensure_path_within(&acme_certs, &paths.riku_root).is_ok()
                || ensure_path_within(&acme_certs, &paths.acme_root).is_ok()
            {
                echo(
                    &format!("--> Removing folder '{}'", acme_certs.display()),
                    "yellow",
                );
                fs::remove_dir_all(&acme_certs)?;
            } else {
                echo(
                    &format!(
                        "WARNING: ACME cert path '{}' points outside expected directories, skipping removal",
                        acme_certs.display()
                    ),
                    "yellow",
                );
            }
        }
        echo(
            &format!("--> Removing file '{}'", acme_link.display()),
            "yellow",
        );
        // acme_link is a symlink, so remove_file works
        let _ = fs::remove_file(&acme_link);
    }

    // Preserve data and cache directories
    for dir in [&paths.data_root, &paths.cache_root] {
        let p = dir.join(&app);
        if p.exists() {
            echo(&format!("==> Preserving folder '{}'", p.display()), "red");
        }
    }

    Ok(())
}

/// Tail app log files using multi_tail.
pub fn cmd_logs(paths: &RikuPaths, app: &str, process: &str) -> Result<()> {
    let app = exit_if_invalid(app, &paths.app_root)?;

    let pattern = paths.log_root.join(&app).join(format!("{}.*.log", process));
    let logfiles: Vec<String> = glob::glob(pattern.to_str().unwrap_or(""))
        .map(|g| {
            g.filter_map(|e| e.ok().map(|p| p.to_string_lossy().to_string()))
                .collect()
        })
        .unwrap_or_default();

    if !logfiles.is_empty() {
        multi_tail(&logfiles)?;
    } else {
        echo(&format!("No logs found for app '{}'.", app), "yellow");
    }
    Ok(())
}

/// Show all processes for all apps.
pub fn cmd_ps_all(paths: &RikuPaths, verbose: bool) -> Result<()> {
    let app_root = &paths.app_root;

    if !app_root.exists() {
        echo("No applications deployed.", "yellow");
        return Ok(());
    }

    let mut apps: Vec<String> = fs::read_dir(app_root)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();

    if apps.is_empty() {
        echo("No applications deployed.", "yellow");
        return Ok(());
    }

    apps.sort();

    if verbose {
        // Show detailed view with stats from supervisor
        let headers = vec!["APP", "PROCESS", "KIND", "PID", "STATUS", "HEALTH"];
        let mut rows: Vec<Vec<String>> = Vec::new();
        let mut total_processes = 0;

        // Try to get stats from supervisor first
        let stats_file = paths.riku_root.join("stats.json");
        let stats_data = if stats_file.exists() {
            fs::read_to_string(&stats_file)
                .ok()
                .and_then(|content| serde_json::from_str::<Vec<serde_json::Value>>(&content).ok())
        } else {
            None
        };

        for app in &apps {
            // Check both .toml and .ini worker configs
            let toml_pattern = paths.workers_enabled.join(format!("{}*.toml", app));
            let ini_pattern = paths.workers_enabled.join(format!("{}*.ini", app));

            let mut worker_configs: Vec<_> = match glob::glob(toml_pattern.to_str().unwrap_or("")) {
                Ok(g) => g.filter_map(|r| r.ok()).collect(),
                Err(e) => {
                    eprintln!("Warning: glob failed for toml worker configs: {}", e);
                    Vec::new()
                }
            };

            let ini_configs: Vec<_> = match glob::glob(ini_pattern.to_str().unwrap_or("")) {
                Ok(g) => g.filter_map(|r| r.ok()).collect(),
                Err(e) => {
                    eprintln!("Warning: glob failed for ini worker configs: {}", e);
                    Vec::new()
                }
            };
            worker_configs.extend(ini_configs);

            for config_path in worker_configs {
                if let Some(filename) = config_path.file_name().and_then(|s| s.to_str()) {
                    // Parse filename: app-kind-ordinal.toml or app-kind-ordinal.ini
                    let stem = filename.trim_end_matches(".toml").trim_end_matches(".ini");
                    let parts: Vec<&str> = stem.split('-').collect();
                    if parts.len() >= 3 {
                        let kind = parts[1];
                        let ordinal = parts.get(2).unwrap_or(&"1");
                        let process_name = format!("{}-{}-{}", app, kind, ordinal);

                        // Get PID, status, and health from stats if available
                        let (pid, status, health) = if let Some(ref stats_vec) = stats_data {
                            let mut pid = "N/A".to_string();
                            let mut status = "unknown".to_string();
                            let mut health = "unknown".to_string();

                            for app_stats in stats_vec {
                                if let Some(processes) =
                                    app_stats.get("processes").and_then(|v| v.as_array())
                                {
                                    for proc_stats in processes {
                                        if let Some(proc_id) =
                                            proc_stats.get("process_id").and_then(|v| v.as_str())
                                        {
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
                        };

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

        crate::util::print_table_with_title("=== All Processes ===", &headers, &rows, 2);

        println!(
            "Total: {} process(es) across {} app(s)",
            total_processes.to_string().green(),
            apps.len().to_string().green()
        );
    } else {
        // Show compact view
        let headers = vec!["APP", "WORKERS"];
        let mut rows: Vec<Vec<String>> = Vec::new();

        for app in &apps {
            // Count both .toml and .ini worker configs
            let toml_pattern = paths.workers_enabled.join(format!("{}*.toml", app));
            let ini_pattern = paths.workers_enabled.join(format!("{}*.ini", app));

            let toml_count = match glob::glob(toml_pattern.to_str().unwrap_or("")) {
                Ok(g) => g.count(),
                Err(_) => 0,
            };
            let ini_count = match glob::glob(ini_pattern.to_str().unwrap_or("")) {
                Ok(g) => g.count(),
                Err(_) => 0,
            };
            let worker_count = toml_count + ini_count;

            let prefix = if worker_count > 0 { "*" } else { " " };
            rows.push(vec![
                format!("{}{}", prefix, app),
                format!("{} worker(s)", worker_count),
            ]);
        }

        crate::util::print_table_with_title("=== Deployed Apps ===", &headers, &rows, 2);

        println!();
        echo(
            "Use 'riku ps <app> --verbose' for detailed process info",
            "yellow",
        );
    }

    Ok(())
}

/// Show process scaling info.
pub fn cmd_ps_show(paths: &RikuPaths, app: &str, verbose: bool) -> Result<()> {
    let app = exit_if_invalid(app, &paths.app_root)?;

    // Check for running worker configs (both .toml and .ini)
    let toml_pattern = paths.workers_enabled.join(format!("{}*.toml", app));
    let ini_pattern = paths.workers_enabled.join(format!("{}*.ini", app));
    let mut worker_configs: Vec<_> = match glob::glob(toml_pattern.to_str().unwrap_or("")) {
        Ok(g) => g.filter_map(|r| r.ok()).collect(),
        Err(e) => {
            eprintln!("Warning: glob failed for worker configs: {}", e);
            Vec::new()
        }
    };
    let ini_configs: Vec<_> = match glob::glob(ini_pattern.to_str().unwrap_or("")) {
        Ok(g) => g.filter_map(|r| r.ok()).collect(),
        Err(e) => {
            eprintln!("Warning: glob failed for ini worker configs: {}", e);
            Vec::new()
        }
    };
    worker_configs.extend(ini_configs);

    if verbose && worker_configs.is_empty() {
        // For verbose mode with no running processes, show desired scale from SCALING file
        let config_file = paths.env_root.join(&app).join("SCALING");
        if config_file.exists() {
            let content = fs::read_to_string(&config_file)?;
            let headers = vec!["KIND", "DESIRED", "STATUS"];
            let mut rows: Vec<Vec<String>> = Vec::new();

            for line in content.lines() {
                let line = line.trim();
                if !line.is_empty() && !line.starts_with('#') {
                    if let Some(pos) = line.find('=') {
                        let kind = line[..pos].trim().to_string();
                        let count = line[pos + 1..].trim().to_string();
                        rows.push(vec![kind.clone(), count, "stopped".to_string()]);
                    }
                }
            }

            if !rows.is_empty() {
                crate::util::print_table_with_title(
                    &format!("=== Processes for '{}' (stopped) ===", app),
                    &headers,
                    &rows,
                    2,
                );
                return Ok(());
            }
        }

        echo(
            &format!("No processes configured for app '{}'.", app),
            "yellow",
        );
        return Ok(());
    }

    if worker_configs.is_empty() {
        // Non-verbose mode: just show scaling config
        let config_file = paths.env_root.join(&app).join("SCALING");
        if config_file.exists() {
            let content = fs::read_to_string(&config_file)?;
            let headers = vec!["KIND", "DESIRED"];
            let mut rows: Vec<Vec<String>> = Vec::new();

            for line in content.lines() {
                let line = line.trim();
                if !line.is_empty() && !line.starts_with('#') {
                    if let Some(pos) = line.find('=') {
                        let kind = line[..pos].trim().to_string();
                        let count = line[pos + 1..].trim().to_string();
                        rows.push(vec![kind, count]);
                    }
                }
            }

            if !rows.is_empty() {
                crate::util::print_table_with_title(
                    &format!("=== Scaling for '{}' (stopped) ===", app),
                    &headers,
                    &rows,
                    2,
                );
                return Ok(());
            }
        }

        echo(
            &format!("No running processes found for app '{}'.", app),
            "yellow",
        );
        return Ok(());
    }

    if verbose {
        // Show detailed process info with stats from supervisor
        let headers = vec!["PROCESS", "KIND", "PID", "STATUS", "HEALTH"];
        let mut rows: Vec<Vec<String>> = Vec::new();

        // Try to get stats from supervisor first
        let stats_file = paths.riku_root.join("stats.json");
        let stats_data = if stats_file.exists() {
            fs::read_to_string(&stats_file)
                .ok()
                .and_then(|content| serde_json::from_str::<Vec<serde_json::Value>>(&content).ok())
        } else {
            None
        };

        for config_path in worker_configs {
            if let Some(filename) = config_path.file_name().and_then(|s| s.to_str()) {
                // Parse filename: app-kind-ordinal.toml or app-kind-ordinal.ini
                let stem = filename.trim_end_matches(".toml").trim_end_matches(".ini");
                let parts: Vec<&str> = stem.split('-').collect();
                if parts.len() >= 3 {
                    let kind = parts[1];
                    let ordinal = parts.get(2).unwrap_or(&"1");
                    let process_name = format!("{}-{}-{}", app, kind, ordinal);

                    // Get PID and status from stats if available
                    let (pid, status, health) = if let Some(ref stats_vec) = stats_data {
                        let mut pid = "N/A".to_string();
                        let mut status = "unknown".to_string();
                        let mut health = "unknown".to_string();

                        for app_stats in stats_vec {
                            if let Some(processes) =
                                app_stats.get("processes").and_then(|v| v.as_array())
                            {
                                for proc_stats in processes {
                                    if let Some(proc_id) =
                                        proc_stats.get("process_id").and_then(|v| v.as_str())
                                    {
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
                    };

                    rows.push(vec![process_name, kind.to_string(), pid, status, health]);
                }
            }
        }

        crate::util::print_table_with_title(
            &format!("=== Processes for '{}' ===", app),
            &headers,
            &rows,
            2,
        );
    } else {
        // Show simple scaling info
        let headers = vec!["KIND", "COUNT"];
        let mut rows: Vec<Vec<String>> = Vec::new();

        let config_file = paths.env_root.join(&app).join("SCALING");
        if config_file.exists() {
            let content = fs::read_to_string(&config_file)?;
            for line in content.lines() {
                let line = line.trim();
                if !line.is_empty() && !line.starts_with('#') {
                    if let Some(pos) = line.find('=') {
                        let kind = line[..pos].trim().to_string();
                        let count = line[pos + 1..].trim().to_string();
                        rows.push(vec![kind, count]);
                    }
                }
            }
        } else {
            // Count workers from enabled configs
            let mut counts: HashMap<String, u32> = HashMap::new();
            for config_path in &worker_configs {
                if let Some(filename) = config_path.file_name().and_then(|s| s.to_str()) {
                    let parts: Vec<&str> = filename.trim_end_matches(".toml").split('-').collect();
                    if parts.len() >= 2 {
                        let kind = parts[1].to_string();
                        *counts.entry(kind).or_insert(0) += 1;
                    }
                }
            }

            for (kind, count) in counts {
                rows.push(vec![kind, count.to_string()]);
            }
        }

        crate::util::print_table_with_title(
            &format!("=== Scaling for '{}' ===", app),
            &headers,
            &rows,
            2,
        );
    }

    Ok(())
}

/// Scale workers — parse SCALING file, compute deltas, deploy.
pub fn cmd_ps_scale(paths: &RikuPaths, app: &str, settings: &[String]) -> Result<()> {
    let app = exit_if_invalid(app, &paths.app_root)?;

    let config_file = paths.env_root.join(&app).join("SCALING");
    // Parse the scaling file as a procfile-style k:v format
    let mut env = HashMap::new();
    let worker_count = parse_settings(&config_file, &mut env)?;

    let mut deltas: HashMap<String, i64> = HashMap::new();
    for s in settings {
        if let Some(eq_pos) = s.find('=') {
            let k = s[..eq_pos].trim().to_string();
            let v_str = s[eq_pos + 1..].trim().to_string();
            match v_str.parse::<i64>() {
                Ok(c) => {
                    if c < 0 {
                        echo(&format!("Error: cannot scale type '{}' below 0", k), "red");
                        return Ok(());
                    }
                    if let Some(current) = worker_count.get(&k) {
                        match current.parse::<i64>() {
                            Ok(current_val) => {
                                deltas.insert(k, c - current_val);
                            }
                            Err(_) => {
                                echo(&format!("Error: malformed setting '{}'", s), "red");
                                return Ok(());
                            }
                        }
                    } else {
                        // Worker type not present - allow adding new types
                        echo(
                            &format!("Adding new worker type '{}' with count {}", k, c),
                            "green",
                        );
                        deltas.insert(k, c); // Positive delta adds new workers
                    }
                }
                Err(_) => {
                    echo(&format!("Error: malformed setting '{}'", s), "red");
                    return Ok(());
                }
            }
        } else {
            echo(&format!("Error: malformed setting '{}'", s), "red");
            return Ok(());
        }
    }

    // Call do_deploy with the calculated deltas
    crate::deploy::do_deploy(&app, paths, &deltas, None)?;
    Ok(())
}

/// Run a command in the app context with LIVE_ENV loaded.
pub fn cmd_run(paths: &RikuPaths, app: &str, cmd: &[String]) -> Result<()> {
    let app = exit_if_invalid(app, &paths.app_root)?;

    let config_file = paths.env_root.join(&app).join("LIVE_ENV");
    let mut env = HashMap::new();
    parse_settings(&config_file, &mut env)?;

    let app_dir = paths.app_root.join(&app);
    let shell_cmd = cmd.join(" ");

    let mut child = Command::new("sh")
        .arg("-c")
        .arg(&shell_cmd)
        .current_dir(&app_dir)
        .envs(&env)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?;

    child.wait()?;
    Ok(())
}

/// Restart an app: stop then spawn.
pub fn cmd_restart(paths: &RikuPaths, app: &str) -> Result<()> {
    let app = exit_if_invalid(app, &paths.app_root)?;

    echo(&format!("restarting app '{}'...", app), "yellow");
    do_stop(paths, &app);
    // Trigger a deploy to restart the app
    let deltas: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    crate::deploy::do_deploy(&app, paths, &deltas, None)
}

/// Stop an app by removing enabled worker configs.
pub fn cmd_stop(paths: &RikuPaths, app: &str) -> Result<()> {
    let app = exit_if_invalid(app, &paths.app_root)?;
    do_stop(paths, &app);
    Ok(())
}

/// Self-update the binary by downloading latest from RIKU_RAW_SOURCE_URL.
pub fn cmd_update() -> Result<()> {
    echo("Updating riku...", "");

    let output = Command::new("curl")
        .args([
            "-sL",
            "-w",
            "%{http_code}",
            RIKU_RAW_SOURCE_URL,
            "-o",
            "/dev/null",
        ])
        .output()?;

    let http_code = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if http_code == "200" {
        echo(
            "Note: self-update for riku binary is not yet implemented.",
            "yellow",
        );
        echo("The piku.py reference source is accessible.", "");
    } else {
        echo(
            &format!(
                "Error updating riku - please check if {} is accessible from this machine.",
                RIKU_RAW_SOURCE_URL
            ),
            "",
        );
    }
    echo("Done.", "");
    Ok(())
}

/// Start the supervisor daemon.
/// Note: For production use, use 'riku supervisor --daemon' or systemd service.
pub fn cmd_supervisor(paths: &RikuPaths) -> Result<()> {
    let mut supervisor = Supervisor::new(paths.workers_enabled.clone())?;
    supervisor.run()
}

// --- Internal helpers ---

/// Stop an app by removing its enabled worker config files.
fn do_stop(paths: &RikuPaths, app: &str) {
    let mut configs: Vec<std::path::PathBuf> = Vec::new();

    for ext in &["ini", "toml"] {
        let pattern = paths.workers_enabled.join(format!("{}*.{}", app, ext));
        if let Ok(entries) = glob::glob(pattern.to_str().unwrap_or("")) {
            for entry in entries.flatten() {
                configs.push(entry);
            }
        }
    }

    if !configs.is_empty() {
        echo(&format!("Stopping app '{}'...", app), "yellow");
        for c in &configs {
            let _ = fs::remove_file(c);
        }
    } else {
        echo(&format!("Error: app '{}' not deployed!", app), "red");
    }
}

/// Simple shell-like splitting of a string on whitespace, respecting quotes.
fn shell_split(input: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut escape_next = false;

    for c in input.chars() {
        if escape_next {
            current.push(c);
            escape_next = false;
            continue;
        }
        if c == '\\' && !in_single_quote {
            escape_next = true;
            continue;
        }
        if c == '\'' && !in_double_quote {
            in_single_quote = !in_single_quote;
            continue;
        }
        if c == '"' && !in_single_quote {
            in_double_quote = !in_double_quote;
            continue;
        }
        if c.is_whitespace() && !in_single_quote && !in_double_quote {
            if !current.is_empty() {
                result.push(current.clone());
                current.clear();
            }
            continue;
        }
        current.push(c);
    }
    if !current.is_empty() {
        result.push(current);
    }
    result
}

/// Tail multiple log files, showing the last `catch_up` lines then polling.
fn multi_tail(filenames: &[String]) -> Result<()> {
    let catch_up: usize = 20;

    // Compute prefixes (filename stem without extension)
    let prefixes: Vec<String> = filenames
        .iter()
        .map(|f| {
            Path::new(f)
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        })
        .collect();

    let longest = prefixes.iter().map(|p| p.len()).max().unwrap_or(0);

    // Catch up: show last `catch_up` lines from each file
    for (i, f) in filenames.iter().enumerate() {
        if let Ok(file) = fs::File::open(f) {
            let reader = BufReader::new(file);
            #[allow(clippy::lines_filter_map_ok)]
            let lines: VecDeque<String> = reader
                .lines()
                .filter_map(Result::ok)
                .collect::<VecDeque<String>>();
            // Take last catch_up lines
            let start = if lines.len() > catch_up {
                lines.len() - catch_up
            } else {
                0
            };
            for line in lines.iter().skip(start) {
                println!(
                    "{}",
                    format!(
                        "{} | {}",
                        prefixes[i].as_str().to_string()
                            + &" ".repeat(longest.saturating_sub(prefixes[i].len())),
                        line
                    )
                    .white()
                );
            }
        }
    }

    // Open files at the end for tailing
    let mut files: Vec<fs::File> = Vec::new();
    let mut inodes: Vec<u64> = Vec::new();
    for f in filenames {
        let mut file = fs::File::open(f)?;
        let meta = file.metadata()?;
        inodes.push(meta.ino());
        file.seek(SeekFrom::End(0))?;
        files.push(file);
    }

    let mut active_filenames: Vec<String> = filenames.to_vec();

    loop {
        let mut updated = false;

        for i in 0..active_filenames.len() {
            let mut buf = String::new();
            if files[i].read_to_string(&mut buf).is_ok() && !buf.is_empty() {
                updated = true;
                for line in buf.lines() {
                    println!(
                        "{}",
                        format!("{:<width$} | {}", prefixes[i], line, width = longest).white()
                    );
                }
            }
        }

        if !updated {
            thread::sleep(Duration::from_secs(1));
            // Check for log rotation
            let mut i = 0;
            while i < active_filenames.len() {
                let f = &active_filenames[i];
                if Path::new(f).exists() {
                    if let Ok(meta) = fs::metadata(f) {
                        if meta.ino() != inodes[i] {
                            // Log rotated, reopen
                            if let Ok(mut new_file) = fs::File::open(f) {
                                let _ = new_file.seek(SeekFrom::Start(0));
                                files[i] = new_file;
                                inodes[i] = meta.ino();
                            }
                        }
                    }
                    i += 1;
                } else {
                    active_filenames.remove(i);
                    files.remove(i);
                    inodes.remove(i);
                    // Don't increment i since we removed an element
                }
            }
            if active_filenames.is_empty() {
                break;
            }
        }
    }

    Ok(())
}

/// Show stats for all apps.
pub fn cmd_stats_all(paths: &RikuPaths) -> Result<()> {
    // Read stats from the supervisor's stats file if it exists
    let stats_file = paths.riku_root.join("stats.json");

    if stats_file.exists() {
        if let Ok(content) = fs::read_to_string(&stats_file) {
            if let Ok(stats_vec) = serde_json::from_str::<Vec<serde_json::Value>>(&content) {
                println!("{}", "=== Riku Stats ===".green());
                println!();

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

                        println!("{}", format!("App: {}", app).green());
                        println!("  Processes: {}/{} running", running_procs, total_procs);
                        println!("  Healthy: {}/{}", healthy_procs, total_procs);
                        println!("  Memory: {:.2} MB", memory_mb);
                        println!();
                    }
                }
                return Ok(());
            }
        }
    }

    // Fallback: show basic info from worker configs
    println!("{}", "=== Deployed Apps ===".green());
    println!();

    if !paths.app_root.exists() {
        echo("No apps deployed.", "yellow");
        return Ok(());
    }

    for entry in fs::read_dir(&paths.app_root)?.flatten() {
        let app_name = entry.file_name().to_string_lossy().to_string();

        // Count workers
        let toml_pattern = paths.workers_enabled.join(format!("{}*.toml", app_name));
        let worker_count = glob::glob(toml_pattern.to_str().unwrap_or(""))
            .map(|g| g.count())
            .unwrap_or(0);

        println!("{}: {} workers", app_name.green(), worker_count);
    }

    println!();
    println!("Note: Detailed stats require supervisor to be running.");

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
                            println!("{}", format!("=== Stats for '{}' ===", app).green());
                            println!();

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
    println!("{}", format!("=== Processes for '{}' ===", app).green());

    let toml_pattern = paths.workers_enabled.join(format!("{}*.toml", app));
    let worker_configs: Vec<_> = glob::glob(toml_pattern.to_str().unwrap_or(""))
        .map(|g| g.filter_map(|r| r.ok()).collect())
        .unwrap_or_else(|_| Vec::new());

    if worker_configs.is_empty() {
        echo("No running processes found.", "yellow");
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

/// Hot reload an app (zero downtime restart).
pub fn cmd_hot_reload(paths: &RikuPaths, app: &str) -> Result<()> {
    let app = exit_if_invalid(app, &paths.app_root)?;

    echo(&format!("Hot reloading app '{}'...", app), "green");

    // Signal the supervisor by updating the mtime of each enabled worker TOML.
    // The supervisor's file watcher (notify) detects the Modify event and reloads the config.
    // We achieve a real mtime bump by reading the content and writing it back.
    let toml_pattern = paths.workers_enabled.join(format!("{}*.toml", app));

    if let Ok(entries) = glob::glob(toml_pattern.to_str().unwrap_or("")) {
        let mut count = 0;
        for entry in entries.flatten() {
            // Read and rewrite the file to bump its mtime, triggering a supervisor reload.
            match fs::read_to_string(&entry) {
                Ok(content) => {
                    if let Err(e) = fs::write(&entry, content) {
                        echo(
                            &format!("Warning: failed to touch {}: {}", entry.display(), e),
                            "yellow",
                        );
                    } else {
                        count += 1;
                    }
                }
                Err(e) => {
                    echo(
                        &format!("Warning: failed to read {}: {}", entry.display(), e),
                        "yellow",
                    );
                }
            }
        }

        if count > 0 {
            echo(
                &format!("Triggered hot reload for {} worker(s)", count),
                "green",
            );
            echo(
                "Note: Supervisor must be running for hot reload to take effect.",
                "yellow",
            );
        } else {
            echo("No worker configs found. Is the app deployed?", "yellow");
        }
    }

    Ok(())
}

/// Create a new application (directory and git repository).
pub fn cmd_apps_create(paths: &RikuPaths, name: &str) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let app = sanitize_app_name(name);

    // Check if app already exists
    if paths.app_root.join(&app).exists() {
        echo(&format!("Error: app '{}' already exists.", app), "red");
        return Ok(());
    }

    // Create app directory
    let app_dir = paths.app_root.join(&app);
    fs::create_dir_all(&app_dir)?;
    echo(
        &format!("✓ Created app directory: {}", app_dir.display()),
        "green",
    );

    // Create git repository
    let repo_dir = paths.git_root.join(format!("{}.git", app));
    fs::create_dir_all(&repo_dir)?;

    // Initialize bare git repo
    Command::new("git")
        .args(["init", "--bare"])
        .current_dir(&repo_dir)
        .output()?;

    echo(
        &format!("✓ Created git repository: {}", repo_dir.display()),
        "green",
    );

    // Create post-receive hook
    let hooks_dir = repo_dir.join("hooks");
    fs::create_dir_all(&hooks_dir)?;

    let post_receive = hooks_dir.join("post-receive");
    let hook_script = format!(
        r#"#!/bin/bash
# Riku post-receive hook for app: {}

while read oldrev newrev refname; do
    RIKU_BIN="$HOME/.local/bin/riku"
    if [ -x "$RIKU_BIN" ]; then
        # Get the actual repo path
        REPO_PATH="$(pwd)"
        "$RIKU_BIN" git-hook "{}" "$REPO_PATH"
    else
        echo " !     Riku binary not found at $RIKU_BIN"
    fi
done
"#,
        app, app
    );

    fs::write(&post_receive, hook_script)?;
    fs::set_permissions(&post_receive, PermissionsExt::from_mode(0o755))?;

    echo(
        &format!("✓ Created git hook: {}", post_receive.display()),
        "green",
    );
    echo("", "");

    echo(&format!("App '{}' created successfully!", app), "green");
    echo("", "");
    echo("Deploy your code:", "yellow");
    echo(
        &format!("  git remote add riku deploy@your-server:{}", app),
        "yellow",
    );
    echo("  git push riku main", "yellow");
    echo("", "");

    Ok(())
}

/// Show detailed information about an application.
pub fn cmd_apps_info(paths: &RikuPaths, app: &str) -> Result<()> {
    let app = sanitize_app_name(app);
    let app_dir = paths.app_root.join(&app);

    // Check if app exists
    if !app_dir.exists() {
        echo(&format!("Error: app '{}' not found.", app), "red");
        bail!("App not found");
    }

    println!("{}", format!("=== App: '{}' ===", app).green());
    println!();

    // App directory
    println!("App Directory: {}", app_dir.display());

    // Git remote
    let git_dir = paths.git_root.join(format!("{}.git", app));
    if git_dir.exists() {
        println!("Git Repository: {}", git_dir.display());
        println!("Git Remote: deploy@your-server:{}", app);
    }

    // Disk usage
    if let Some(app_dir_str) = app_dir.to_str() {
        if let Ok(output) = Command::new("du").args(["-sh", app_dir_str]).output() {
            if let Ok(du_output) = String::from_utf8(output.stdout) {
                if let Some(size) = du_output.split_whitespace().next() {
                    println!("Disk Usage: {}", size);
                }
            }
        }
    }

    // Environment variables summary
    let env_file = paths.env_root.join(&app).join("ENV");
    if env_file.exists() {
        let mut env = HashMap::new();
        parse_settings(&env_file, &mut env)?;
        let var_count = env.len();
        println!("Environment Variables: {} configured", var_count);

        // Show key variables
        for key in ["NGINX_SERVER_NAME", "NODE_VERSION", "PORT"] {
            if let Some(val) = env.get(key) {
                println!("  {}: {}", key, val);
            }
        }
    }

    // Scaling config
    let scaling_file = paths.env_root.join(&app).join("SCALING");
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

    // Process status
    let toml_pattern = paths.workers_enabled.join(format!("{}*.toml", app));
    let ini_pattern = paths.workers_enabled.join(format!("{}*.ini", app));

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

        // Try to get detailed stats from supervisor
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
    } else {
        println!("Status: {}", "stopped".yellow());
    }

    // Nginx config
    let nginx_conf = paths.nginx_root.join(format!("{}.conf", app));
    if nginx_conf.exists() {
        println!("Nginx Config: {}", nginx_conf.display());
    }

    // Logs
    let log_dir = paths.log_root.join(&app);
    if log_dir.exists() {
        println!("Log Directory: {}", log_dir.display());
    }

    // Data directory (if exists)
    let data_dir = paths.data_root.join(&app);
    if data_dir.exists() {
        println!("Data Directory: {}", data_dir.display());
    }

    Ok(())
}
