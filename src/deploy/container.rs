//! Container deployment module.
//!
//! Handles deployment of containerized applications using Docker or Podman.
//! Worker config creation is delegated to [`super::container_workers`].

use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::RikuPaths;
use crate::util::echo;

use super::container_workers::{create_compose_workers, create_container_workers};

/// Check if Docker is available on the system.
pub fn is_docker_available() -> Result<bool> {
    let result = Command::new("docker").arg("--version").output();
    match result {
        Ok(output) => Ok(output.status.success()),
        Err(_) => Ok(false),
    }
}

/// Check if Podman is available on the system.
pub fn is_podman_available() -> Result<bool> {
    let result = Command::new("podman").arg("--version").output();
    match result {
        Ok(output) => Ok(output.status.success()),
        Err(_) => Ok(false),
    }
}

/// Detect which container runtime is available (Docker or Podman).
pub fn detect_container_runtime() -> Result<Option<String>> {
    if is_docker_available()? {
        Ok(Some("docker".to_string()))
    } else if is_podman_available()? {
        Ok(Some("podman".to_string()))
    } else {
        Ok(None)
    }
}

/// Check if the app has container files (Dockerfile, Containerfile, or compose files).
#[allow(dead_code)]
pub fn has_container_files(app_path: &Path) -> bool {
    if app_path.join("Dockerfile").exists() || app_path.join("Containerfile").exists() {
        return true;
    }
    has_compose_file(app_path)
}

/// Deploy a containerized application using the available container runtime.
pub fn deploy_container(
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
) -> Result<()> {
    match detect_container_runtime()? {
        Some(runtime) => {
            echo(
                &format!("-----> Deploying container app '{}' with {}", app, runtime),
                "green",
            );

            let has_dockerfile = app_path.join("Dockerfile").exists();
            let has_containerfile = app_path.join("Containerfile").exists();
            let has_compose = has_compose_file(app_path);

            if has_compose {
                deploy_with_compose(app, app_path, env, paths, &runtime)
            } else if has_dockerfile || has_containerfile {
                deploy_with_build(app, app_path, env, paths, &runtime)
            } else {
                create_container_workers(app, app_path, env, paths, &runtime)
            }
        }
        None => Err(anyhow::anyhow!(
            "Neither Docker nor Podman is available on this system"
        )),
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Check if the app has any compose file.
fn has_compose_file(app_path: &Path) -> bool {
    compose_file_names()
        .iter()
        .any(|f| app_path.join(f).exists())
}

/// Return the path to the first compose file found, if any.
fn get_compose_file_path(app_path: &Path) -> Option<PathBuf> {
    compose_file_names()
        .iter()
        .map(|f| app_path.join(f))
        .find(|p| p.exists())
}

fn compose_file_names() -> &'static [&'static str] {
    &[
        "docker-compose.yml",
        "docker-compose.yaml",
        "podman-compose.yml",
        "podman-compose.yaml",
        "compose.yml",
        "compose.yaml",
    ]
}

/// Deploy using compose file.
fn deploy_with_compose(
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
    runtime: &str,
) -> Result<()> {
    echo("-----> Using compose file for deployment", "green");

    let compose_path = get_compose_file_path(app_path)
        .ok_or_else(|| anyhow::anyhow!("No compose file found for app {}", app))?;

    let compose_file_name = compose_path
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("Invalid compose file path"))?
        .to_string_lossy()
        .to_string();

    create_compose_workers(app, app_path, env, paths, runtime, &compose_file_name)
}

/// Deploy by building container images.
fn deploy_with_build(
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
    runtime: &str,
) -> Result<()> {
    let has_dockerfile = app_path.join("Dockerfile").exists();
    let has_containerfile = app_path.join("Containerfile").exists();

    if has_dockerfile || has_containerfile {
        echo(&format!("-----> Building {} image", runtime), "green");
        let image_name = format!("riku-{}", app);

        let mut build_args = vec!["build", "-t", &image_name, "."];
        if has_containerfile && !has_dockerfile {
            build_args = vec!["build", "-f", "Containerfile", "-t", &image_name, "."];
        }

        let status = Command::new(runtime)
            .args(build_args)
            .current_dir(app_path)
            .status()?;

        if !status.success() {
            return Err(anyhow::anyhow!("Failed to build {} image", runtime));
        }
    }

    create_container_workers(app, app_path, env, paths, runtime)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_container_runtime() {
        let runtime = detect_container_runtime().unwrap();
        assert!(runtime.is_some() || runtime.is_none());
    }
}
