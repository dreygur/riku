//! Worker configuration creation for Node.js applications.

use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::config::RikuPaths;
use crate::deploy::read_scaling_count;
use crate::setup_web_port;
use crate::util::echo;
use crate::write_worker_config;

/// Remove existing worker configs for the app when `RIKU_AUTO_RESTART` is enabled.
fn remove_existing_workers(app: &str, paths: &RikuPaths, env: &HashMap<String, String>) {
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
}

/// Create worker configurations for Node.js applications.
pub(super) fn create_node_workers(
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
) -> Result<()> {
    remove_existing_workers(app, paths, env);

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
pub(super) fn create_node_worker_config(
    app: &str,
    kind: &str,
    command: &str,
    ordinal: u32,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
    app_path: &Path,
) -> Result<()> {
    let mut worker_env = env.clone();

    let final_command = if kind == "web" {
        let port = setup_web_port!(worker_env, app, paths);

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

    worker_env.insert("NODE_ENV".to_string(), "production".to_string());

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
    use tempfile::TempDir;

    #[test]
    fn test_create_node_worker_config() {
        let temp_dir = TempDir::new().unwrap();
        let paths = crate::config::RikuPaths::from_dirs(
            temp_dir.path().join(".riku"),
            &temp_dir.path().to_path_buf(),
        );

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

        let config_path = paths.workers_available.join("testapp-web-1.toml");
        assert!(config_path.exists());
    }
}
