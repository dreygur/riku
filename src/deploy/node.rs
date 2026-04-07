//! Node.js application deployment module.
//!
//! Handles deployment of Node.js applications using npm, yarn, or pnpm.
//! Worker configuration creation is delegated to [`super::node_workers`].

use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::config::RikuPaths;
use crate::util::{echo, validate_node_version};

use super::node_workers::create_node_workers;

/// Deploy a Node.js application using npm (or an alternate package manager).
///
/// If the environment variable `RIKU_SKIP_BUILD` is set (to any value), the
/// package-installation and Node.js version-install steps are skipped.  This
/// is intended for tests that run without npm present on the host.
pub fn deploy_node(
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
) -> Result<()> {
    echo(&format!("-----> Deploying Node.js app '{}'", app), "green");

    let skip_build = std::env::var("RIKU_SKIP_BUILD").is_ok();

    if !skip_build {
        install_node_version(app, app_path, env, paths)?;
        isolate_node_modules(app, app_path, env, paths)?;
        install_dependencies(app_path, env)?;
    } else {
        echo("-----> Skipping build steps (RIKU_SKIP_BUILD set)", "yellow");
    }

    create_node_workers(app, app_path, env, paths)?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Install a specific Node.js version via nodeenv if `NODE_VERSION` is set.
fn install_node_version(
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
) -> Result<()> {
    let node_version = match env.get("NODE_VERSION") {
        Some(v) => v.clone(),
        None => return Ok(()),
    };

    if let Err(e) = validate_node_version(&node_version) {
        return Err(anyhow::anyhow!("Invalid NODE_VERSION: {}", e));
    }

    echo(
        &format!("-----> Installing Node.js version {}", node_version),
        "green",
    );

    if which::which("nodeenv").is_ok() {
        let nodeenv_path = paths.env_root.join(app).join("nodeenv");
        if !nodeenv_path.exists() {
            let status = Command::new("nodeenv")
                .arg(&nodeenv_path)
                .arg("-n")
                .arg(&node_version)
                .current_dir(app_path)
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

    Ok(())
}

/// Create an isolated `node_modules` directory in `ENV_ROOT` and symlink it.
fn isolate_node_modules(
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
) -> Result<()> {
    let package_manager = env
        .get("NODE_PACKAGE_MANAGER")
        .cloned()
        .unwrap_or_else(|| "npm".to_string());

    let node_modules_path = paths.env_root.join(app).join("node_modules");
    let should_isolate = package_manager != "yarn" && !node_modules_path.exists();

    if should_isolate {
        echo("-----> Creating isolated node_modules in ENV_ROOT", "green");
        fs::create_dir_all(&node_modules_path)?;

        let app_node_modules = app_path.join("node_modules");
        if app_node_modules
            .symlink_metadata()
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false)
        {
            let _ = fs::remove_file(&app_node_modules);
        }
        if !app_node_modules.exists() {
            let _ = std::os::unix::fs::symlink(&node_modules_path, &app_node_modules);
        }
    }

    Ok(())
}

/// Install npm/yarn/pnpm dependencies.
fn install_dependencies(app_path: &Path, env: &HashMap<String, String>) -> Result<()> {
    let package_manager = env
        .get("NODE_PACKAGE_MANAGER")
        .cloned()
        .unwrap_or_else(|| "npm".to_string());

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
        if which::which("pnpm").is_err() {
            let _ = Command::new("npm")
                .args(["install", "-g", "pnpm"])
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

    Ok(())
}
