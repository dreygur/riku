//! Node.js application deployment module.
//!
//! Handles deployment of Node.js applications using npm or yarn.

use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::config::RikuPaths;
use crate::deploy::read_scaling_count;
use crate::setup_web_port;
use crate::util::echo;
use crate::write_worker_config;

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

    // Create isolated node_modules in ENV_ROOT if not using yarn (yarn handles this differently)
    let node_modules_path = paths.env_root.join(app).join("node_modules");
    let should_isolate = package_manager != "yarn" && !node_modules_path.exists();

    if should_isolate {
        echo("-----> Creating isolated node_modules in ENV_ROOT", "green");
        fs::create_dir_all(&node_modules_path)?;

        // Create symlink from app directory to ENV_ROOT node_modules
        let app_node_modules = app_path.join("node_modules");
        if app_node_modules.exists() {
            // Remove existing node_modules if it's a symlink
            if app_node_modules
                .symlink_metadata()
                .map(|m| m.file_type().is_symlink())
                .unwrap_or(false)
            {
                let _ = fs::remove_file(&app_node_modules);
            }
        }
        if !app_node_modules.exists() {
            let _ = std::os::unix::fs::symlink(&node_modules_path, &app_node_modules);
        }
    }

    // Install dependencies
    echo(
        &format!("-----> Installing dependencies with {}", package_manager),
        "green",
    );

    let install_result = if package_manager == "yarn" {
        Command::new("yarn")
            .arg("install")
            .current_dir(app_path)
            .output()
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
            .output()
    } else {
        Command::new("npm")
            .arg("install")
            .current_dir(app_path)
            .output()
    };

    match install_result {
        Ok(output) => {
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                let stdout = String::from_utf8_lossy(&output.stdout);
                echo(&format!("npm install stdout: {}", stdout), "yellow");
                echo(&format!("npm install stderr: {}", stderr), "red");
                return Err(anyhow::anyhow!("Failed to install dependencies"));
            }
            // Show success message with package count if available
            let stdout = String::from_utf8_lossy(&output.stdout);
            if stdout.contains("added") {
                echo(
                    &format!(
                        "-----> {}",
                        stdout
                            .lines()
                            .find(|l| l.contains("added"))
                            .unwrap_or("Dependencies installed")
                    ),
                    "green",
                );
            }
        }
        Err(e) => {
            echo(&format!("Failed to run {}: {}", package_manager, e), "red");
            return Err(anyhow::anyhow!("Failed to run package manager"));
        }
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
    // Handle RIKU_AUTO_RESTART - if false, skip removing existing worker configs
    let auto_restart = env
        .get("RIKU_AUTO_RESTART")
        .map(|v| v.to_lowercase() != "false" && v != "0" && v != "no")
        .unwrap_or(true);

    if auto_restart {
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

    let procfile_content = fs::read_to_string(&procfile_path)?;
    for line in procfile_content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some(pos) = line.find(':') {
            let kind = line[..pos].trim();
            let command = line[pos + 1..].trim();

            let count = read_scaling_count(paths, app, kind)?;

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
    let mut worker_env = env.clone();

    // Set PORT for web processes and determine final command
    let final_command = if kind == "web" {
        let port = setup_web_port!(worker_env, app, paths);

        // If the command doesn't already specify a port, inject it via PORT= prefix
        if command.contains("--port") || command.contains("PORT=") {
            command.to_string()
        } else if command.contains("node")
            && (command.contains(".js") || command.contains("server"))
        {
            format!("PORT={} {}", port, command)
        } else {
            command.to_string()
        }
    } else {
        command.to_string()
    };

    // Add Node-specific environment variables
    worker_env.insert("NODE_ENV".to_string(), "production".to_string());

    // Add NODE_PATH for isolated node_modules in ENV_ROOT
    let node_modules_path = paths.env_root.join(app).join("node_modules");
    if node_modules_path.exists() {
        worker_env.insert(
            "NODE_PATH".to_string(),
            node_modules_path.to_string_lossy().to_string(),
        );
    }

    write_worker_config!(
        app,
        kind,
        &final_command,
        ordinal,
        worker_env,
        app_path,
        paths
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
