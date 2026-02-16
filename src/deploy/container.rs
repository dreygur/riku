//! Container deployment module.
//!
//! Handles deployment of containerized applications using Docker or Podman.

use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::RikuPaths;
use crate::supervisor::config::create_worker_config;
use crate::util::{echo, get_free_port};

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
    // Check for Dockerfile or Containerfile
    if app_path.join("Dockerfile").exists() || app_path.join("Containerfile").exists() {
        return true;
    }

    // Check for compose files (docker-compose, podman-compose, or standard compose)
    let compose_files = [
        "docker-compose.yml",
        "docker-compose.yaml",
        "podman-compose.yml",
        "podman-compose.yaml",
        "compose.yml",
        "compose.yaml",
    ];

    for file in &compose_files {
        if app_path.join(file).exists() {
            return true;
        }
    }

    false
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

            // Check if we have compose files
            let has_dockerfile = app_path.join("Dockerfile").exists();
            let has_containerfile = app_path.join("Containerfile").exists();
            let has_compose = has_compose_file(app_path);

            if has_compose {
                // Use compose file for deployment
                deploy_with_compose(app, app_path, env, paths, &runtime)
            } else if has_dockerfile || has_containerfile {
                // Build and run individual containers
                deploy_with_build(app, app_path, env, paths, &runtime)
            } else {
                // No build files, assume image is already available
                create_container_workers(app, app_path, env, paths, &runtime)
            }
        }
        None => Err(anyhow::anyhow!(
            "Neither Docker nor Podman is available on this system"
        )),
    }
}

/// Check if the app has any compose file.
fn has_compose_file(app_path: &Path) -> bool {
    let compose_files = [
        "docker-compose.yml",
        "docker-compose.yaml",
        "podman-compose.yml",
        "podman-compose.yaml",
        "compose.yml",
        "compose.yaml",
    ];

    for file in &compose_files {
        if app_path.join(file).exists() {
            return true;
        }
    }

    false
}

/// Get the compose file path for the app.
fn get_compose_file_path(app_path: &Path) -> Option<PathBuf> {
    let compose_files = [
        "docker-compose.yml",
        "docker-compose.yaml",
        "podman-compose.yml",
        "podman-compose.yaml",
        "compose.yml",
        "compose.yaml",
    ];

    for file in &compose_files {
        let path = app_path.join(file);
        if path.exists() {
            return Some(path);
        }
    }

    None
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

    // For compose-based deployments, we create a special worker that runs the compose service
    // This is different from individual container deployments
    create_compose_workers(app, app_path, env, paths, runtime)
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

        // If using Containerfile, specify it explicitly
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

    // Create worker configurations for containerized processes
    create_container_workers(app, app_path, env, paths, runtime)
}

/// Deploy a containerized application using Docker.
#[allow(dead_code)]
fn deploy_docker_internal(
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
    runtime_cmd: &str,
) -> Result<()> {
    // Check if Dockerfile or docker-compose.yml exists
    let has_dockerfile = app_path.join("Dockerfile").exists();
    let has_compose = app_path.join("docker-compose.yml").exists()
        || app_path.join("docker-compose.yaml").exists();

    if has_dockerfile {
        echo("-----> Building Docker image", "green");
        let image_name = format!("riku-{}", app);

        let status = Command::new(runtime_cmd)
            .args(["build", "-t", &image_name, "."])
            .current_dir(app_path)
            .status()?;

        if !status.success() {
            return Err(anyhow::anyhow!("Failed to build {} image", runtime_cmd));
        }
    } else if has_compose {
        echo("-----> Using docker-compose configuration", "green");
        // For compose files, we'll create a special worker config that runs docker-compose
    }

    // Create worker configurations for containerized processes
    create_container_workers(app, app_path, env, paths, runtime_cmd)
}

/// Deploy a containerized application using Podman.
#[allow(dead_code)]
fn deploy_podman_internal(
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
    runtime_cmd: &str,
) -> Result<()> {
    // Check if Containerfile or podman-compose.yml exists
    let has_containerfile =
        app_path.join("Containerfile").exists() || app_path.join("Dockerfile").exists();
    let has_compose = app_path.join("podman-compose.yml").exists()
        || app_path.join("podman-compose.yaml").exists()
        || app_path.join("docker-compose.yml").exists()
        || app_path.join("docker-compose.yaml").exists();

    if has_containerfile {
        echo("-----> Building Podman image", "green");
        let image_name = format!("riku-{}", app);

        let status = Command::new(runtime_cmd)
            .args(["build", "-t", &image_name, "."])
            .current_dir(app_path)
            .status()?;

        if !status.success() {
            return Err(anyhow::anyhow!("Failed to build {} image", runtime_cmd));
        }
    } else if has_compose {
        echo("-----> Using podman-compose configuration", "green");
        // For compose files, we'll create a special worker config that runs podman-compose
    }

    // Create worker configurations for containerized processes
    create_container_workers(app, app_path, env, paths, runtime_cmd)
}

/// Deploy a pre-built container image from a tar archive.
#[allow(dead_code)]
pub fn deploy_container_export(
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
    image_tar_path: &str,
) -> Result<()> {
    match detect_container_runtime()? {
        Some(runtime) => {
            echo(
                &format!(
                    "-----> Loading container image for app '{}' with {}",
                    app, runtime
                ),
                "green",
            );

            // Load the exported image
            let status = Command::new(&runtime)
                .args(["load", "-i", image_tar_path])
                .status()?;

            if !status.success() {
                return Err(anyhow::anyhow!(
                    "Failed to load {} image from archive",
                    runtime
                ));
            }

            // Create worker configurations for the imported image
            create_container_workers(app, app_path, env, paths, &runtime)
        }
        None => Err(anyhow::anyhow!(
            "Neither Docker nor Podman is available on this system"
        )),
    }
}

/// Deploy a locally built container image to remote.
#[allow(dead_code)]
pub fn deploy_local_image(
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
    image_name: &str,
) -> Result<()> {
    match detect_container_runtime()? {
        Some(runtime) => {
            echo(
                &format!(
                    "-----> Deploying local image '{}' for app '{}' with {}",
                    image_name, app, runtime
                ),
                "green",
            );

            // Tag the image with a unique name for this app if needed
            let tagged_image = format!("riku-{}", app);
            let status = Command::new(&runtime)
                .args(["tag", image_name, &tagged_image])
                .status()?;

            if !status.success() {
                return Err(anyhow::anyhow!("Failed to tag {} image", runtime));
            }

            // Create worker configurations for the image
            create_container_workers_from_image(app, &tagged_image, app_path, env, paths, &runtime)
        }
        None => Err(anyhow::anyhow!(
            "Neither Docker nor Podman is available on this system"
        )),
    }
}

/// Build container image locally and deploy it.
#[allow(dead_code)]
pub fn build_and_deploy_locally(
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
    build_context: &str,
) -> Result<()> {
    match detect_container_runtime()? {
        Some(runtime) => {
            echo(
                &format!(
                    "-----> Building and deploying app '{}' locally with {}",
                    app, runtime
                ),
                "green",
            );

            // Build the image
            let image_name = format!("riku-{}", app);
            let status = Command::new(&runtime)
                .args(["build", "-t", &image_name, build_context])
                .current_dir(app_path)
                .status()?;

            if !status.success() {
                return Err(anyhow::anyhow!("Failed to build {} image", runtime));
            }

            // Create worker configurations for the built image
            create_container_workers_from_image(app, &image_name, app_path, env, paths, &runtime)
        }
        None => Err(anyhow::anyhow!(
            "Neither Docker nor Podman is available on this system"
        )),
    }
}

/// Create worker configurations for containerized processes.
#[allow(dead_code)]
fn create_container_workers(
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
    runtime: &str,
) -> Result<()> {
    // Read Procfile to determine processes to run
    let procfile_path = app_path.join("Procfile");
    let workers = crate::util::parse_procfile(&procfile_path);

    let workers = match workers {
        Some(w) if !w.is_empty() => w,
        _ => {
            // If no Procfile, default to a web process running the container
            let mut default_workers = HashMap::new();
            let image_name = format!("riku-{}", app);
            default_workers.insert(
                "web".to_string(),
                format!("{} run --rm -p ${{PORT}}:80 {}", runtime, image_name),
            );
            default_workers
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
            create_container_worker_config(app, kind, command, i, env, paths, app_path, runtime)?;
        }
    }

    Ok(())
}

/// Create worker configurations for a specific container image.
#[allow(dead_code)]
fn create_container_workers_from_image(
    app: &str,
    image_name: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
    runtime: &str,
) -> Result<()> {
    // Read Procfile to determine processes to run
    let procfile_path = app_path.join("Procfile");
    let workers = crate::util::parse_procfile(&procfile_path);

    let workers = match workers {
        Some(w) if !w.is_empty() => w,
        _ => {
            // If no Procfile, default to a web process
            let mut default_workers = HashMap::new();
            default_workers.insert(
                "web".to_string(),
                format!("{} run --rm -p ${{PORT}}:80 {}", runtime, image_name),
            );
            default_workers
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
            create_container_worker_config(app, kind, command, i, env, paths, app_path, runtime)?;
        }
    }

    Ok(())
}

/// Create worker configurations for compose-based deployments.
fn create_compose_workers(
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
    runtime: &str,
) -> Result<()> {
    // Get the compose file path
    let compose_path = get_compose_file_path(app_path)
        .ok_or_else(|| anyhow::anyhow!("No compose file found for app {}", app))?;

    // Get the compose file name without extension to determine the command
    let compose_file_name = compose_path
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("Invalid compose file path"))?
        .to_string_lossy()
        .to_string();

    // Read Procfile to determine processes to run, or default to compose services
    let procfile_path = app_path.join("Procfile");
    let workers = crate::util::parse_procfile(&procfile_path);

    let workers = match workers {
        Some(w) if !w.is_empty() => w,
        _ => {
            // If no Procfile, we'll create a default web service using compose
            let mut default_workers = HashMap::new();
            let compose_cmd = format!("{} compose -f {} up", runtime, compose_file_name);
            default_workers.insert("web".to_string(), compose_cmd);
            default_workers
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
            create_container_worker_config(app, kind, command, i, env, paths, app_path, runtime)?;
        }
    }

    Ok(())
}

/// Create a single worker configuration for a container process.
#[allow(clippy::too_many_arguments)]
fn create_container_worker_config(
    app: &str,
    kind: &str,
    command: &str,
    ordinal: u32,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
    app_path: &Path,
    _runtime: &str,
) -> Result<()> {
    // Prepare environment for the worker
    let mut worker_env = env.clone();

    // Set PORT for web processes and determine final command
    let final_command = if kind == "web" {
        let port = get_free_port("127.0.0.1");
        worker_env.insert("PORT".to_string(), port.to_string());

        // Create socket file for web processes
        let socket_path = paths.nginx_root.join(format!("{}.sock", app));
        worker_env.insert(
            "SOCKET".to_string(),
            socket_path.to_string_lossy().to_string(),
        );

        // Update command to include port mapping if it's a container command
        if command.contains("docker run") || command.contains("podman run") {
            // Replace $PORT in the command with the actual port
            command.replace("${PORT}", &port.to_string())
        } else if command.contains("compose") {
            // For compose commands, we might need to pass the port as an environment variable
            format!("PORT={} {}", port, command)
        } else {
            command.to_string()
        }
    } else {
        command.to_string()
    };

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
        &format!(
            "-----> Created container worker config: {}",
            config_filename
        ),
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
    fn test_detect_container_runtime() {
        // This test will depend on what's available on the system
        let runtime = detect_container_runtime().unwrap();
        // The result depends on what's installed, so we just check it returns Ok
        assert!(runtime.is_some() || runtime.is_none());
    }
}
