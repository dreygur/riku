//! Go application deployment module.
//!
//! Handles deployment of Go applications.

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

/// Deploy a Go application.
pub fn deploy_go(
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
) -> Result<()> {
    echo(&format!("-----> Deploying Go app '{}'", app), "green");

    // Check for Godeps directory (legacy dep support)
    let godeps_path = app_path.join("Godeps");
    let go_path = app_path.join("vendor");
    let go_mod_path = app_path.join("go.mod");

    // If using go modules (go.mod exists)
    if go_mod_path.exists() {
        echo("-----> Building Go application (modules)", "green");

        // Set GO15VENDOREXPERIMENT if vendor directory exists
        let mut go_env = env.clone();
        if go_path.exists() {
            go_env.insert("GO15VENDOREXPERIMENT".to_string(), "1".to_string());
        }

        let status = Command::new("go")
            .arg("build")
            .arg("-mod=vendor")
            .arg("-o")
            .arg(format!("{}_bin", app))
            .current_dir(app_path)
            .envs(go_env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
            .status()?;

        if !status.success() {
            // Try without vendor flag if it failed
            let status = Command::new("go")
                .arg("build")
                .arg("-o")
                .arg(format!("{}_bin", app))
                .current_dir(app_path)
                .envs(go_env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
                .status()?;

            if !status.success() {
                return Err(anyhow::anyhow!("Failed to build Go application"));
            }
        }
    } else if godeps_path.exists() {
        // Legacy: using godep
        echo("-----> Building Go application (godep)", "green");

        // Check if godep is available
        if which::which("godep").is_ok() {
            let status = Command::new("godep")
                .arg("go")
                .arg("build")
                .arg("-o")
                .arg(format!("{}_bin", app))
                .current_dir(app_path)
                .status()?;

            if !status.success() {
                return Err(anyhow::anyhow!("Failed to build Go application with godep"));
            }
        } else {
            echo("-----> godep not found, using standard go build", "yellow");
            let status = Command::new("go")
                .arg("build")
                .arg("-o")
                .arg(format!("{}_bin", app))
                .current_dir(app_path)
                .status()?;

            if !status.success() {
                return Err(anyhow::anyhow!("Failed to build Go application"));
            }
        }
    } else {
        // Standard go build
        echo("-----> Building Go application", "green");
        let status = Command::new("go")
            .arg("build")
            .arg("-o")
            .arg(format!("{}_bin", app))
            .current_dir(app_path)
            .status()?;

        if !status.success() {
            return Err(anyhow::anyhow!("Failed to build Go application"));
        }
    }

    // Create worker configurations
    create_go_workers(app, app_path, env, paths)?;

    Ok(())
}

/// Create worker configurations for Go applications.
fn create_go_workers(
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
                create_go_worker_config(app, kind, command, i, env, paths, app_path)?;
            }
        }
    }

    Ok(())
}

/// Create a single worker configuration for a Go process.
fn create_go_worker_config(
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

        // If the binary doesn't already accept a port flag, append -port=
        if command.contains("--port") || command.contains("PORT=") {
            command.to_string()
        } else if command.ends_with("_bin") {
            format!("{} -port={}", command, port)
        } else {
            command.to_string()
        }
    } else {
        command.to_string()
    };

    // Add Go-specific environment variables
    worker_env.insert("GOROOT".to_string(), String::new()); // picked up from environment

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
    fn test_create_go_worker_config() {
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

        let result = create_go_worker_config(
            "testapp",
            "web",
            "./testapp_bin",
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
