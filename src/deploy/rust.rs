//! Rust application deployment module.
//!
//! Handles deployment of Rust applications using Cargo.

use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::config::RikuPaths;
use crate::setup_web_port;
use crate::util::echo;
use crate::write_worker_config;

/// Deploy a Rust application using Cargo.
pub fn deploy_rust(
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
) -> Result<()> {
    echo(&format!("-----> Deploying Rust app '{}'", app), "green");

    // Build the Rust application in release mode
    echo("-----> Building Rust application", "green");
    let build_status = Command::new("cargo")
        .arg("build")
        .arg("--release")
        .current_dir(app_path)
        .status()?;

    if !build_status.success() {
        return Err(anyhow::anyhow!("Failed to build Rust application"));
    }

    // Create worker configurations
    create_rust_workers(app, app_path, env, paths)?;

    Ok(())
}

/// Create worker configurations for Rust applications.
fn create_rust_workers(
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
) -> Result<()> {
    use crate::util::parse_procfile;

    // Handle RIKU_AUTO_RESTART - if false, skip removing existing worker configs
    let auto_restart = env
        .get("RIKU_AUTO_RESTART")
        .map(|v| v.to_lowercase() != "false" && v != "0" && v != "no")
        .unwrap_or(true);

    if auto_restart {
        // Remove existing worker configs to trigger restart
        for ext in &["toml", "ini"] {
            let pattern = paths.workers_enabled.join(format!("{}-*.{}", app, ext));
            if let Ok(entries) = glob::glob(pattern.to_str().unwrap_or("")) {
                for entry in entries.flatten() {
                    let _ = fs::remove_file(&entry);
                }
            }
        }
    }

    // Read Procfile to determine processes to run
    let procfile_path = app_path.join("Procfile");
    let workers = parse_procfile(&procfile_path);
    let workers = match workers {
        Some(w) => w,
        None => {
            echo("-----> No Procfile found, using default", "yellow");
            // Default to running the binary with the app name
            let mut default_workers = HashMap::new();
            default_workers.insert(
                "web".to_string(),
                format!("./target/release/{}", app.replace('-', "_")),
            );
            return create_rust_worker_configs(app, app_path, paths, &default_workers);
        }
    };

    create_rust_worker_configs(app, app_path, paths, &workers)
}

/// Create worker configurations from parsed workers.
fn create_rust_worker_configs(
    app: &str,
    app_path: &Path,
    paths: &RikuPaths,
    workers: &HashMap<String, String>,
) -> Result<()> {
    for (kind, command) in workers {
        if kind == "release" || kind == "preflight" {
            continue; // Skip release and preflight commands as they're run once during deploy
        }

        // Read app ENV file fresh for this worker (Rust deployer reads it per-worker)
        let env_file = paths.env_root.join(app).join("ENV");
        let mut app_env: HashMap<String, String> = HashMap::new();
        if env_file.exists() {
            crate::util::parse_settings(&env_file, &mut app_env)?;
        }

        // Parse scaling count: first check SCALING file, then RIKU_WORKER_PROCESSES env var
        let scaling_path = paths.env_root.join(app).join("SCALING");
        let mut count = 1u32;
        if scaling_path.exists() {
            let content = fs::read_to_string(&scaling_path)?;
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                if let Some(pos) = line.find('=') {
                    if line[..pos].trim() == kind {
                        if let Ok(n) = line[pos + 1..].trim().parse::<u32>() {
                            count = n;
                        }
                        break;
                    }
                }
            }
        }

        // RIKU_WORKER_PROCESSES overrides the SCALING file (format: "web=2,worker=1")
        if let Some(worker_processes) = app_env.get("RIKU_WORKER_PROCESSES") {
            for proc_def in worker_processes.split(',') {
                let proc_def = proc_def.trim();
                if let Some(eq_pos) = proc_def.find('=') {
                    if proc_def[..eq_pos].trim() == kind {
                        if let Ok(n) = proc_def[eq_pos + 1..].trim().parse::<u32>() {
                            count = n;
                        }
                        break;
                    }
                }
            }
        }

        // Create worker configs for each instance
        for i in 1..=count {
            // Re-read ENV per instance so each worker gets a fresh copy
            let mut worker_env: HashMap<String, String> = HashMap::new();
            let env_file = paths.env_root.join(app).join("ENV");
            if env_file.exists() {
                crate::util::parse_settings(&env_file, &mut worker_env)?;
            }

            let final_command = if kind == "web" {
                setup_web_port!(worker_env, app, paths);
                command.clone()
            } else {
                command.clone()
            };

            write_worker_config!(app, kind, &final_command, i, worker_env, app_path, paths);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_create_rust_worker_config() {
        let temp_dir = TempDir::new().unwrap();
        let app_path = temp_dir.path().join("testapp");
        fs::create_dir(&app_path).unwrap();

        let mut env = HashMap::new();
        env.insert("PORT".to_string(), "5000".to_string());

        let paths = crate::config::RikuPaths::from_dirs(
            temp_dir.path().join(".riku"),
            &temp_dir.path().to_path_buf(),
        );
        fs::create_dir_all(&paths.workers_available).unwrap();
        fs::create_dir_all(&paths.workers_enabled).unwrap();
        fs::create_dir_all(&paths.log_root.join("testapp")).unwrap();

        let mut workers = HashMap::new();
        workers.insert("web".to_string(), "./target/release/testapp".to_string());

        let result = create_rust_worker_configs("testapp", &app_path, &paths, &workers);
        assert!(result.is_ok());

        // Check that worker config was created
        let config_file = paths.workers_available.join("testapp-web-1.toml");
        assert!(config_file.exists());

        // Check that symlink was created
        let enabled_file = paths.workers_enabled.join("testapp-web-1.toml");
        assert!(enabled_file.exists());
    }
}
