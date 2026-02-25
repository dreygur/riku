//! Rust application deployment module.
//!
//! Handles deployment of Rust applications using Cargo.

use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::config::RikuPaths;
use crate::supervisor::config::create_worker_config;
use crate::util::{echo, get_free_port};

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

    // Handle PIKU_AUTO_RESTART - if false, skip removing existing worker configs
    let auto_restart = env
        .get("PIKU_AUTO_RESTART")
        .map(|v| v.to_lowercase() != "false" && v != "0" && v != "no")
        .unwrap_or(true);

    if auto_restart {
        // Remove existing worker configs to trigger restart
        for ext in &["toml", "ini"] {
            let pattern = paths.workers_enabled.join(format!("{}*.{}", app, ext));
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
            return create_rust_worker_configs(app, app_path, env, paths, &default_workers);
        }
    };

    create_rust_worker_configs(app, app_path, env, paths, &workers)
}

/// Create worker configurations from parsed workers.
fn create_rust_worker_configs(
    app: &str,
    app_path: &Path,
    _env: &HashMap<String, String>,
    paths: &RikuPaths,
    workers: &HashMap<String, String>,
) -> Result<()> {
    for (kind, command) in workers {
        if kind == "release" || kind == "preflight" {
            continue; // Skip release and preflight commands as they're run once during deploy
        }

        // Parse scaling info if available
        let scaling_path = paths.env_root.join(app).join("SCALING");
        let mut count = 1; // default to 1 instance

        if scaling_path.exists() {
            let scaling_content = fs::read_to_string(&scaling_path)?;
            for scale_line in scaling_content.lines() {
                let scale_line = scale_line.trim();
                if scale_line.is_empty() || scale_line.starts_with('#') {
                    continue;
                }

                if let Some(scale_pos) = scale_line.find('=') {
                    let scale_kind = scale_line[..scale_pos].trim();
                    let scale_count_str = scale_line[scale_pos + 1..].trim();

                    if scale_kind == kind {
                        if let Ok(scale_count) = scale_count_str.parse::<u32>() {
                            count = scale_count;
                        }
                        break;
                    }
                }
            }
        }

        // Check for RIKU_WORKER_PROCESSES env var (format: "web=2,worker=1")
        let env_file = paths.env_root.join(app).join("ENV");
        let mut app_env: HashMap<String, String> = HashMap::new();
        if env_file.exists() {
            crate::util::parse_settings(&env_file, &mut app_env)?;
        }

        if let Some(worker_processes) = app_env.get("RIKU_WORKER_PROCESSES") {
            for proc_def in worker_processes.split(',') {
                let proc_def = proc_def.trim();
                if let Some(eq_pos) = proc_def.find('=') {
                    let proc_kind = proc_def[..eq_pos].trim();
                    let proc_count_str = proc_def[eq_pos + 1..].trim();

                    if proc_kind == kind {
                        if let Ok(proc_count) = proc_count_str.parse::<u32>() {
                            count = proc_count;
                        }
                        break;
                    }
                }
            }
        }

        // Create worker configs for each instance
        for i in 1..=count {
            // Prepare environment for the worker
            let env_file = paths.env_root.join(app).join("ENV");
            let mut worker_env: HashMap<String, String> = HashMap::new();
            if env_file.exists() {
                crate::util::parse_settings(&env_file, &mut worker_env)?;
            }

            // Set PORT for web processes
            let final_command = command.clone();
            if kind == "web" {
                let port = get_free_port("127.0.0.1")?;
                worker_env.insert("PORT".to_string(), port.to_string());

                // Create socket file for web processes (kept for backwards compatibility)
                let socket_path = paths.nginx_root.join(format!("{}.sock", app));
                worker_env.insert(
                    "SOCKET".to_string(),
                    socket_path.to_string_lossy().to_string(),
                );

                // Set NGINX_PORTMAP to use TCP proxying instead of unix socket
                worker_env.insert("NGINX_PORTMAP".to_string(), "true".to_string());
                worker_env.insert("NGINX_INTERNAL_PORT".to_string(), port.to_string());
                worker_env.insert("NGINX_EXTERNAL_PORT".to_string(), "80".to_string());

                // Write NGINX settings to ENV file for nginx config generation
                let env_dir = paths.env_root.join(app);
                fs::create_dir_all(&env_dir)?;
                let env_file = env_dir.join("ENV");

                let mut env_content = if env_file.exists() {
                    fs::read_to_string(&env_file)?
                } else {
                    String::new()
                };

                if !env_content.contains("NGINX_PORTMAP") {
                    env_content.push_str(&format!("NGINX_PORTMAP=true\n"));
                    env_content.push_str(&format!("NGINX_INTERNAL_PORT={}\n", port));
                    env_content.push_str("NGINX_EXTERNAL_PORT=80\n");
                    fs::write(&env_file, &env_content)?;
                }
            }

            // Create the worker config
            let worker_config = create_worker_config(
                app,
                kind,
                &final_command,
                i,
                worker_env,
                &app_path.to_string_lossy(),
                &paths
                    .log_root
                    .join(app)
                    .join(format!("{}.{}.log", kind, i))
                    .to_string_lossy(),
            );

            // Write the worker config to the available directory
            let config_filename = format!("{}-{}-{}.toml", app, kind, i);
            let config_path = paths.workers_available.join(&config_filename);

            let config_content = toml::to_string(&worker_config)?;
            fs::write(&config_path, &config_content)?;

            // Create a symlink to enable the worker
            let enabled_path = paths.workers_enabled.join(&config_filename);
            if enabled_path.exists() {
                fs::remove_file(&enabled_path)?;
            }
            std::os::unix::fs::symlink(&config_path, &enabled_path)?;

            echo(
                &format!("-----> Created worker config: {}", config_filename),
                "green",
            );
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

        let result = create_rust_worker_configs("testapp", &app_path, &env, &paths, &workers);
        assert!(result.is_ok());

        // Check that worker config was created
        let config_file = paths.workers_available.join("testapp-web-1.toml");
        assert!(config_file.exists());

        // Check that symlink was created
        let enabled_file = paths.workers_enabled.join("testapp-web-1.toml");
        assert!(enabled_file.exists());
    }
}
