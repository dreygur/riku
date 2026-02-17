//! Node.js application deployment module.
//!
//! Handles deployment of Node.js applications using npm or yarn.

use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::config::RikuPaths;
use crate::supervisor::config::create_worker_config;
use crate::util::{echo, get_free_port};

/// Deploy a Node.js application using npm.
pub fn deploy_node(
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
) -> Result<()> {
    echo(&format!("-----> Deploying Node.js app '{}'", app), "green");

    // Handle NODE_VERSION - install specific Node version if requested
    if let Some(node_version) = env.get("NODE_VERSION") {
        echo(
            &format!("-----> Installing Node.js version {}", node_version),
            "green",
        );

        // Check if nodeenv is available
        if which::which("nodeenv").is_ok() {
            let nodeenv_path = paths.env_root.join(app).join("nodeenv");
            if !nodeenv_path.exists() {
                let status = Command::new("nodeenv")
                    .arg(&nodeenv_path)
                    .arg("-n")
                    .arg(node_version)
                    .status()?;

                if !status.success() {
                    echo(
                        "-----> Failed to install nodeenv, using system Node.js",
                        "yellow",
                    );
                } else {
                    echo(
                        &format!("-----> Node.js {} installed via nodeenv", node_version),
                        "green",
                    );
                }
            }
        } else {
            echo("-----> nodeenv not found, using system Node.js", "yellow");
        }
    }

    // Determine package manager
    let package_manager = env
        .get("NODE_PACKAGE_MANAGER")
        .cloned()
        .unwrap_or_else(|| "npm".to_string());

    // Install dependencies
    echo(
        &format!("-----> Installing dependencies with {}", package_manager),
        "green",
    );

    let install_status = if package_manager == "yarn" {
        Command::new("yarn")
            .arg("install")
            .current_dir(app_path)
            .status()
    } else if package_manager == "pnpm" {
        // Install pnpm if not available
        if which::which("pnpm").is_err() {
            let _ = Command::new("npm")
                .arg("install")
                .arg("-g")
                .arg("pnpm")
                .status();
        }
        Command::new("pnpm")
            .arg("install")
            .current_dir(app_path)
            .status()
    } else {
        Command::new("npm")
            .arg("install")
            .current_dir(app_path)
            .status()
    };

    if let Ok(status) = install_status {
        if !status.success() {
            return Err(anyhow::anyhow!("Failed to install dependencies"));
        }
    } else {
        return Err(anyhow::anyhow!("Failed to run package manager"));
    }

    // Create worker configurations
    create_node_workers(app, app_path, env, paths)?;

    Ok(())
}

/// Create worker configurations for Node.js applications.
fn create_node_workers(
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
) -> Result<()> {
    // Read Procfile to determine processes to run
    let procfile_path = app_path.join("Procfile");
    if !procfile_path.exists() {
        echo(
            "-----> No Procfile found, skipping process creation",
            "yellow",
        );
        return Ok(());
    }

    let procfile_content = fs::read_to_string(&procfile_path)?;
    for line in procfile_content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some(pos) = line.find(':') {
            let kind = line[..pos].trim();
            let command = line[pos + 1..].trim();

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
                create_node_worker_config(app, kind, command, i, env, paths, app_path)?;
            }
        }
    }

    Ok(())
}

/// Create a single worker configuration for a Node.js process.
fn create_node_worker_config(
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

    // Set PORT for web processes and determine final command
    let final_command = if kind == "web" {
        let port = get_free_port("127.0.0.1").expect("Failed to find a free port for web process");
        worker_env.insert("PORT".to_string(), port.to_string());

        // Update command to include port if it's a web process
        let updated_command = if command.contains("--port") || command.contains("PORT=") {
            command.to_string()
        } else {
            // If it's a common Node.js server, add port binding
            if command.contains("node") && (command.contains(".js") || command.contains("server")) {
                format!("PORT={} {}", port, command)
            } else {
                command.to_string()
            }
        };

        // Create socket file for web processes
        let socket_path = paths.nginx_root.join(format!("{}.sock", app));
        worker_env.insert(
            "SOCKET".to_string(),
            socket_path.to_string_lossy().to_string(),
        );

        updated_command
    } else {
        command.to_string()
    };

    // Add Node-specific environment variables
    worker_env.insert("NODE_ENV".to_string(), "production".to_string());

    // Create the worker config
    let worker_config = create_worker_config(
        app,
        kind,
        &final_command,
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
    fn test_create_node_worker_config() {
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

        let result = create_node_worker_config(
            "testapp",
            "web",
            "node server.js",
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
