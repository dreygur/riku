//! Worker configuration creation for Python applications.
//!
//! Handles cleaning existing configs, reading the Procfile, and writing
//! TOML worker configs for pip, Poetry, and uv-based deployments.

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
pub(super) fn remove_existing_workers(app: &str, paths: &RikuPaths, env: &HashMap<String, String>) {
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

/// Create worker configurations for pip-based Python applications.
pub(super) fn create_python_workers(
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
    python_env_path: &Path,
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
                create_python_worker_config(
                    app,
                    kind,
                    command,
                    i,
                    env,
                    paths,
                    python_env_path,
                    app_path,
                )?;
            }
        }
    }

    Ok(())
}

/// Create worker configurations for Poetry/uv apps, wrapping each Procfile
/// command with `runner` (e.g. "poetry run" or "uv run").
pub(super) fn create_python_workers_with_runner(
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
    runner: &str,
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
            let wrapped = format!("{} {}", runner, command);

            let count = read_scaling_count(paths, app, kind)?;
            for i in 1..=count {
                let mut worker_env = env.clone();

                let final_command = if kind == "web" {
                    let port = setup_web_port!(worker_env, app, paths);
                    if wrapped.contains("--bind") || wrapped.contains("--port") {
                        wrapped.clone()
                    } else if wrapped.contains("gunicorn") {
                        format!("{} --bind 127.0.0.1:{}", wrapped, port)
                    } else if wrapped.contains("flask") {
                        format!("{} run --host=127.0.0.1 --port={}", wrapped, port)
                    } else if wrapped.contains("uvicorn") {
                        format!("{} --host 127.0.0.1 --port {}", wrapped, port)
                    } else {
                        wrapped.clone()
                    }
                } else {
                    wrapped.clone()
                };

                worker_env.insert("PYTHONUNBUFFERED".to_string(), "1".to_string());
                worker_env.insert("PYTHONIOENCODING".to_string(), "UTF-8".to_string());
                worker_env.insert(
                    "PYTHONPATH".to_string(),
                    app_path.to_string_lossy().to_string(),
                );

                write_worker_config!(app, kind, &final_command, i, worker_env, app_path, paths);
            }
        }
    }

    Ok(())
}

/// Create a single worker configuration for a pip-based Python process.
#[allow(clippy::too_many_arguments)]
pub(super) fn create_python_worker_config(
    app: &str,
    kind: &str,
    command: &str,
    ordinal: u32,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
    python_env_path: &Path,
    app_path: &Path,
) -> Result<()> {
    let mut worker_env = env.clone();

    let final_command = if kind == "web" {
        let port = setup_web_port!(worker_env, app, paths);

        if command.contains("--bind") || command.contains("--port") {
            command.to_string()
        } else if command.contains("gunicorn") {
            format!("{} --bind 127.0.0.1:{}", command, port)
        } else if command.contains("flask") {
            format!("{} run --host=127.0.0.1 --port={}", command, port)
        } else if command.contains("uvicorn") {
            format!("{} --host 127.0.0.1 --port {}", command, port)
        } else {
            command.to_string()
        }
    } else {
        command.to_string()
    };

    let bin_path = python_env_path.join("bin");
    let current_path = worker_env.get("PATH").cloned().unwrap_or_default();
    let new_path = if current_path.is_empty() {
        bin_path.to_string_lossy().to_string()
    } else {
        format!("{}:{}", bin_path.to_string_lossy(), current_path)
    };
    worker_env.insert("PATH".to_string(), new_path);

    worker_env.insert(
        "PYTHONPATH".to_string(),
        app_path.to_string_lossy().to_string(),
    );

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
    fn test_create_python_worker_config() {
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

        let result = create_python_worker_config(
            "testapp",
            "web",
            "python app.py",
            1,
            &env,
            &paths,
            temp_dir.path(),
            temp_dir.path(),
        );

        assert!(result.is_ok());

        let config_path = paths.workers_available.join("testapp-web-1.toml");
        assert!(config_path.exists());
    }
}
