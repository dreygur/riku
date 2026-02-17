//! Identity application deployment module.
//!
//! Handles deployment of generic applications that don't require specific build steps.

use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::config::RikuPaths;
use crate::supervisor::config::create_worker_config;
use crate::util::{echo, get_free_port};

/// Deploy an identity application (generic deployment).
#[allow(dead_code)]
pub fn deploy_identity(
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
) -> Result<()> {
    echo(&format!("-----> Deploying identity app '{}'", app), "green");

    // Create worker configurations based on Procfile
    create_identity_workers(app, app_path, env, paths)?;

    Ok(())
}

/// Create worker configurations for identity-style deployments.
#[allow(dead_code)]
pub fn create_identity_workers(
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
    if !procfile_path.exists() {
        echo(
            "-----> No Procfile found, skipping process creation",
            "yellow",
        );
        return Ok(());
    }

    let workers = parse_procfile(&procfile_path);
    let workers = match workers {
        Some(w) => w,
        None => {
            echo("-----> Procfile not found or empty", "yellow");
            return Ok(());
        }
    };

    for (kind, command) in &workers {
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

        // Create worker configs for each instance
        for i in 1..=count {
            create_identity_worker_config(app, kind, command, i, env, paths, app_path)?;
        }
    }

    Ok(())
}

/// Create a single worker configuration for an identity process.
fn create_identity_worker_config(
    app: &str,
    kind: &str,
    command: &str,
    ordinal: u32,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
    app_path: &Path,
) -> Result<()> {
    // Prepare environment for the worker
    let mut worker_env = env.clone();

    // Set PORT for web processes
    if kind == "web" {
        let port = get_free_port("127.0.0.1")
            .expect("Failed to find a free port for web process");
        worker_env.insert("PORT".to_string(), port.to_string());

        // Create socket file for web processes
        let socket_path = paths.nginx_root.join(format!("{}.sock", app));
        worker_env.insert(
            "SOCKET".to_string(),
            socket_path.to_string_lossy().to_string(),
        );
    }

    // Create the worker config
    let worker_config = create_worker_config(
        app,
        kind,
        command,
        ordinal,
        worker_env,
        &app_path.to_string_lossy(),
        &paths
            .log_root
            .join(app)
            .join(format!("{}.{}.log", kind, ordinal))
            .to_string_lossy(),
    );

    // Write the worker config to the available directory
    let config_filename = format!("{}-{}-{}.toml", app, kind, ordinal);
    let config_path = paths.workers_available.join(&config_filename);

    let config_content = toml::to_string(&worker_config)?;
    fs::write(&config_path, config_content)?;

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

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::TempDir;

    #[test]
    fn test_create_identity_worker_config() {
        let temp_dir = TempDir::new().unwrap();
        let paths = crate::config::RikuPaths::from_dirs(
            temp_dir.path().join(".piku"),
            &temp_dir.path().to_path_buf(),
        );

        // Create necessary directories
        fs::create_dir_all(&paths.workers_available).unwrap();
        fs::create_dir_all(&paths.workers_enabled).unwrap();
        fs::create_dir_all(&paths.log_root.join("testapp")).unwrap();

        let mut env = HashMap::new();
        env.insert("ENV_VAR".to_string(), "value".to_string());

        let result = create_identity_worker_config(
            "testapp",
            "web",
            "python app.py",
            1,
            &env,
            &paths,
            temp_dir.path(),
        );

        assert!(result.is_ok());

        // Check that the config file was created
        let config_path = paths.workers_available.join("testapp-web-1.toml");
        assert!(config_path.exists());
    }
}
