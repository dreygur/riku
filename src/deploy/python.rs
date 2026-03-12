//! Python application deployment module.
//!
//! Handles deployment of Python applications using pip, poetry, or uv.

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

/// Deploy a Python application using pip and requirements.txt.
pub fn deploy_python(
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
) -> Result<()> {
    echo(&format!("-----> Deploying Python app '{}'", app), "green");

    // Get Python version from env or default to python3
    let python_version = env.get("PYTHON_VERSION").map(|s| s.as_str()).unwrap_or("3");
    let python_bin = if python_version == "2" {
        "python2"
    } else {
        "python3"
    };

    // Create virtual environment if it doesn't exist
    // Use a `venv/` subdirectory so the venv doesn't collide with ENV/SCALING files
    let venv_path = paths.env_root.join(app).join("venv");
    if !venv_path.exists() {
        echo("-----> Creating virtual environment", "green");
        let status = Command::new(python_bin)
            .arg("-m")
            .arg("venv")
            .arg(&venv_path)
            .current_dir(app_path)
            .status()?;

        if !status.success() {
            return Err(anyhow::anyhow!("Failed to create virtual environment"));
        }
    }

    // Install dependencies
    echo("-----> Installing dependencies", "green");
    let pip_path = venv_path.join("bin").join("pip");
    let status = Command::new(&pip_path)
        .arg("install")
        .arg("--upgrade")
        .arg("-r")
        .arg("requirements.txt")
        .current_dir(app_path)
        .status()?;

    if !status.success() {
        return Err(anyhow::anyhow!("Failed to install dependencies"));
    }

    // Add Python-specific env vars
    let mut python_env = env.clone();
    python_env.insert("PYTHONUNBUFFERED".to_string(), "1".to_string());
    python_env.insert("PYTHONIOENCODING".to_string(), "UTF-8".to_string());
    python_env.insert(
        "VIRTUAL_ENV".to_string(),
        venv_path.to_string_lossy().to_string(),
    );

    // Create worker configurations with modified env
    create_python_workers(app, app_path, &python_env, paths, &venv_path)?;

    Ok(())
}

/// Deploy a Python application using Poetry.
pub fn deploy_python_poetry(
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
) -> Result<()> {
    echo(
        &format!("-----> Deploying Python (Poetry) app '{}'", app),
        "green",
    );

    // Install dependencies with Poetry
    echo("-----> Installing dependencies with Poetry", "green");
    let status = Command::new("poetry")
        .arg("install")
        .current_dir(app_path)
        .status()?;

    if !status.success() {
        return Err(anyhow::anyhow!(
            "Failed to install dependencies with Poetry"
        ));
    }

    // Create worker configurations — wrap every command with `poetry run` so
    // that the Poetry-managed virtualenv is used.  Do NOT prepend {app_path}/bin
    // to PATH; that directory does not exist and would be the wrong venv anyway.
    create_python_workers_with_runner(app, app_path, env, paths, "poetry run")?;

    Ok(())
}

/// Deploy a Python application using uv.
pub fn deploy_python_uv(
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
) -> Result<()> {
    echo(
        &format!("-----> Deploying Python (uv) app '{}'", app),
        "green",
    );

    // Install dependencies with uv
    echo("-----> Installing dependencies with uv", "green");
    let status = Command::new("uv")
        .arg("sync")
        .current_dir(app_path)
        .status()?;

    if !status.success() {
        return Err(anyhow::anyhow!("Failed to install dependencies with uv"));
    }

    // Create worker configurations — wrap every command with `uv run` so that
    // the uv-managed virtualenv is used.  Do NOT prepend {app_path}/bin to PATH.
    create_python_workers_with_runner(app, app_path, env, paths, "uv run")?;

    Ok(())
}

/// Create worker configurations for Python applications.
fn create_python_workers(
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
    python_env_path: &Path,
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
fn create_python_workers_with_runner(
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
    runner: &str,
) -> Result<()> {
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
            // Prefix the command with the runner (e.g. "poetry run gunicorn ...")
            let wrapped = format!("{} {}", runner, command);

            let count = read_scaling_count(paths, app, kind)?;
            for i in 1..=count {
                let mut worker_env = env.clone();

                // Set PORT for web processes
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
                // Do NOT prepend any bin/ directory — the runner (poetry run /
                // uv run) activates the correct virtualenv itself.

                write_worker_config!(app, kind, &final_command, i, worker_env, app_path, paths);
            }
        }
    }

    Ok(())
}

/// Create a single worker configuration for a Python process.
#[allow(clippy::too_many_arguments)]
fn create_python_worker_config(
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

    // Set PORT for web processes and determine final command
    let final_command = if kind == "web" {
        let port = setup_web_port!(worker_env, app, paths);

        // If it's a common Python web server without explicit port args, inject the port
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

    // Prepend the venv bin/ directory to PATH
    let bin_path = python_env_path.join("bin");
    let current_path = worker_env.get("PATH").cloned().unwrap_or_default();
    let new_path = if current_path.is_empty() {
        bin_path.to_string_lossy().to_string()
    } else {
        format!("{}:{}", bin_path.to_string_lossy(), current_path)
    };
    worker_env.insert("PATH".to_string(), new_path);

    // Set PYTHONPATH to app directory
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
    use std::collections::HashMap;
    use tempfile::TempDir;

    #[test]
    fn test_create_python_worker_config() {
        let temp_dir = TempDir::new().unwrap();
        let paths = crate::config::RikuPaths::from_dirs(
            temp_dir.path().join(".riku"),
            &temp_dir.path().to_path_buf(),
        );

        // Create necessary directories
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

        // Check that the config file was created
        let config_path = paths.workers_available.join("testapp-web-1.toml");
        assert!(config_path.exists());
    }
}
