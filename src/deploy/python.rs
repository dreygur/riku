//! Python application deployment module.
//!
//! Handles deployment of Python applications using pip, poetry, or uv.
//! Worker configuration creation is delegated to [`super::python_workers`].

use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use crate::config::RikuPaths;
use crate::util::echo;

use super::python_workers::{
    create_python_workers, create_python_workers_with_runner,
};

/// Deploy a Python application using pip and requirements.txt.
pub fn deploy_python(
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
) -> Result<()> {
    echo(&format!("-----> Deploying Python app '{}'", app), "green");

    let python_version = env.get("PYTHON_VERSION").map(|s| s.as_str()).unwrap_or("3");
    let python_bin = if python_version == "2" { "python2" } else { "python3" };

    let venv_path = paths.env_root.join(app).join("venv");
    if !venv_path.exists() {
        echo("-----> Creating virtual environment", "green");
        let status = Command::new(python_bin)
            .args(["-m", "venv"])
            .arg(&venv_path)
            .current_dir(app_path)
            .status()?;

        if !status.success() {
            return Err(anyhow::anyhow!("Failed to create virtual environment"));
        }
    }

    echo("-----> Installing dependencies", "green");
    let pip_path = venv_path.join("bin").join("pip");
    let status = Command::new(&pip_path)
        .args(["install", "--upgrade", "-r", "requirements.txt"])
        .current_dir(app_path)
        .status()?;

    if !status.success() {
        return Err(anyhow::anyhow!("Failed to install dependencies"));
    }

    let mut python_env = env.clone();
    python_env.insert("PYTHONUNBUFFERED".to_string(), "1".to_string());
    python_env.insert("PYTHONIOENCODING".to_string(), "UTF-8".to_string());
    python_env.insert(
        "VIRTUAL_ENV".to_string(),
        venv_path.to_string_lossy().to_string(),
    );

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

    // Wrap every Procfile command with `poetry run` so the Poetry-managed
    // virtualenv is used. Do NOT prepend {app_path}/bin to PATH.
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

    echo("-----> Installing dependencies with uv", "green");
    let status = Command::new("uv")
        .arg("sync")
        .current_dir(app_path)
        .status()?;

    if !status.success() {
        return Err(anyhow::anyhow!("Failed to install dependencies with uv"));
    }

    // Wrap every Procfile command with `uv run` so the uv-managed virtualenv
    // is used. Do NOT prepend {app_path}/bin to PATH.
    create_python_workers_with_runner(app, app_path, env, paths, "uv run")?;

    Ok(())
}
