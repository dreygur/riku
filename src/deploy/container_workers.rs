//! Worker configuration creation for containerized applications.
//!
//! Handles reading the Procfile, applying scaling counts, and writing
//! TOML worker configs for Docker/Podman-based deployments.

use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::config::RikuPaths;
use crate::supervisor::config::create_worker_config;
use crate::util::{echo, get_free_port};

/// Read scaling count for a worker kind from the SCALING file.
fn read_scaling_count(paths: &RikuPaths, app: &str, kind: &str) -> Result<u32> {
    let scaling_path = paths.env_root.join(app).join("SCALING");
    if !scaling_path.exists() {
        return Ok(1);
    }

    let content = fs::read_to_string(&scaling_path)?;
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(pos) = line.find('=') {
            let scale_kind = line[..pos].trim();
            let scale_count_str = line[pos + 1..].trim();
            if scale_kind == kind {
                if let Ok(n) = scale_count_str.parse::<u32>() {
                    return Ok(n);
                }
            }
        }
    }
    Ok(1)
}

/// Create worker configurations for containerized processes read from Procfile.
///
/// If no Procfile is present or it is empty, a default `web` worker that
/// runs the container image is created instead.
pub(super) fn create_container_workers(
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
    runtime: &str,
) -> Result<()> {
    let procfile_path = app_path.join("Procfile");
    let workers = crate::util::parse_procfile(&procfile_path);

    let workers = match workers {
        Some(w) if !w.is_empty() => w,
        _ => {
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
            continue;
        }

        let count = read_scaling_count(paths, app, kind)?;
        for i in 1..=count {
            create_container_worker_config(app, kind, command, i, env, paths, app_path, runtime)?;
        }
    }

    Ok(())
}

/// Create worker configurations for a compose-based deployment.
pub(super) fn create_compose_workers(
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
    runtime: &str,
    compose_file_name: &str,
) -> Result<()> {
    let procfile_path = app_path.join("Procfile");
    let workers = crate::util::parse_procfile(&procfile_path);

    let workers = match workers {
        Some(w) if !w.is_empty() => w,
        _ => {
            let mut default_workers = HashMap::new();
            let compose_cmd = format!("{} compose -f {} up", runtime, compose_file_name);
            default_workers.insert("web".to_string(), compose_cmd);
            default_workers
        }
    };

    for (kind, command) in &workers {
        if kind == "release" || kind == "preflight" {
            continue;
        }

        let count = read_scaling_count(paths, app, kind)?;
        for i in 1..=count {
            create_container_worker_config(app, kind, command, i, env, paths, app_path, runtime)?;
        }
    }

    Ok(())
}

/// Create worker configurations for a specific pre-tagged container image.
pub(super) fn create_container_workers_from_image(
    app: &str,
    image_name: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
    runtime: &str,
) -> Result<()> {
    let procfile_path = app_path.join("Procfile");
    let workers = crate::util::parse_procfile(&procfile_path);

    let workers = match workers {
        Some(w) if !w.is_empty() => w,
        _ => {
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
            continue;
        }

        let count = read_scaling_count(paths, app, kind)?;
        for i in 1..=count {
            create_container_worker_config(app, kind, command, i, env, paths, app_path, runtime)?;
        }
    }

    Ok(())
}

/// Create a single worker configuration for a container process.
#[allow(clippy::too_many_arguments)]
pub(super) fn create_container_worker_config(
    app: &str,
    kind: &str,
    command: &str,
    ordinal: u32,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
    app_path: &Path,
    _runtime: &str,
) -> Result<()> {
    let mut worker_env = env.clone();

    let final_command = if kind == "web" {
        let port = get_free_port("127.0.0.1")?;
        worker_env.insert("PORT".to_string(), port.to_string());

        let socket_path = paths.nginx_root.join(format!("{}.sock", app));
        worker_env.insert(
            "SOCKET".to_string(),
            socket_path.to_string_lossy().to_string(),
        );

        if command.contains("docker run") || command.contains("podman run") {
            command.replace("${PORT}", &port.to_string())
        } else if command.contains("compose") {
            format!("PORT={} {}", port, command)
        } else {
            command.to_string()
        }
    } else {
        command.to_string()
    };

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

    let config_filename = format!("{}-{}-{}.toml", app, kind, ordinal);
    let config_path = paths.workers_available.join(&config_filename);

    let config_content = toml::to_string(&worker_config)?;
    fs::write(&config_path, &config_content)?;

    let enabled_path = paths.workers_enabled.join(&config_filename);
    if enabled_path.exists() {
        fs::remove_file(&enabled_path)?;
    }
    std::os::unix::fs::symlink(&config_path, &enabled_path)?;

    tracing::info!(app = app, worker = %config_filename, "Created container worker config");
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
    use std::fs;
    use tempfile::TempDir;

    fn make_paths(tmp: &TempDir) -> RikuPaths {
        let paths = crate::config::RikuPaths::from_dirs(
            tmp.path().join(".riku"),
            &tmp.path().to_path_buf(),
        );
        fs::create_dir_all(&paths.workers_available).unwrap();
        fs::create_dir_all(&paths.workers_enabled).unwrap();
        fs::create_dir_all(&paths.nginx_root).unwrap();
        paths
    }

    // --- create_container_worker_config ---

    #[test]
    fn test_create_container_worker_config_creates_toml_file() -> Result<()> {
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);
        fs::create_dir_all(paths.log_root.join("myapp")).unwrap();

        let env = HashMap::new();
        create_container_worker_config(
            "myapp",
            "worker",
            "echo hello",
            1,
            &env,
            &paths,
            tmp.path(),
            "docker",
        )?;

        let config_path = paths.workers_available.join("myapp-worker-1.toml");
        assert!(config_path.exists(), "TOML config file should be created");
        Ok(())
    }

    #[test]
    fn test_create_container_worker_config_creates_symlink_in_enabled() -> Result<()> {
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);
        fs::create_dir_all(paths.log_root.join("myapp")).unwrap();

        let env = HashMap::new();
        create_container_worker_config(
            "myapp",
            "worker",
            "echo hello",
            1,
            &env,
            &paths,
            tmp.path(),
            "podman",
        )?;

        let enabled_path = paths.workers_enabled.join("myapp-worker-1.toml");
        assert!(enabled_path.exists(), "Symlink should be created in workers_enabled");
        Ok(())
    }

    #[test]
    fn test_create_container_worker_config_web_sets_port_env() -> Result<()> {
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);
        fs::create_dir_all(paths.log_root.join("myapp")).unwrap();

        let env = HashMap::new();
        // "web" kind should allocate a PORT
        create_container_worker_config(
            "myapp",
            "web",
            "docker run --rm -p ${PORT}:80 myimage",
            1,
            &env,
            &paths,
            tmp.path(),
            "docker",
        )?;

        let config_path = paths.workers_available.join("myapp-web-1.toml");
        assert!(config_path.exists());
        let content = fs::read_to_string(&config_path)?;
        // The PORT placeholder should have been substituted with a real port number
        assert!(!content.contains("${PORT}"), "PORT placeholder should be substituted");
        Ok(())
    }

    #[test]
    fn test_create_container_worker_config_replaces_existing_symlink() -> Result<()> {
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);
        fs::create_dir_all(paths.log_root.join("myapp")).unwrap();

        let env = HashMap::new();
        // First deploy
        create_container_worker_config(
            "myapp",
            "worker",
            "echo first",
            1,
            &env,
            &paths,
            tmp.path(),
            "docker",
        )?;
        // Second deploy — should not fail even though symlink already exists
        create_container_worker_config(
            "myapp",
            "worker",
            "echo second",
            1,
            &env,
            &paths,
            tmp.path(),
            "docker",
        )?;

        let enabled_path = paths.workers_enabled.join("myapp-worker-1.toml");
        assert!(enabled_path.exists(), "Symlink should still exist after re-deploy");
        Ok(())
    }

    #[test]
    fn test_create_container_worker_config_non_web_keeps_command_unchanged() -> Result<()> {
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);
        fs::create_dir_all(paths.log_root.join("myapp")).unwrap();

        let env = HashMap::new();
        let cmd = "python worker.py";
        create_container_worker_config(
            "myapp",
            "worker",
            cmd,
            1,
            &env,
            &paths,
            tmp.path(),
            "docker",
        )?;

        let config_path = paths.workers_available.join("myapp-worker-1.toml");
        let content = fs::read_to_string(&config_path)?;
        assert!(content.contains(cmd), "Worker command should be preserved as-is");
        Ok(())
    }

    // --- create_container_workers (from Procfile) ---

    #[test]
    fn test_create_container_workers_skips_release_and_preflight() -> Result<()> {
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);
        fs::create_dir_all(paths.log_root.join("myapp")).unwrap();
        fs::create_dir_all(paths.env_root.join("myapp")).unwrap();

        let app_path = tmp.path().join("app");
        fs::create_dir_all(&app_path).unwrap();
        fs::write(
            app_path.join("Procfile"),
            "release: ./migrate\npreflight: ./healthcheck\nworker: python w.py\n",
        )?;

        let env = HashMap::new();
        create_container_workers("myapp", &app_path, &env, &paths, "docker")?;

        // Only worker config should exist, not release or preflight
        assert!(!paths.workers_available.join("myapp-release-1.toml").exists());
        assert!(!paths.workers_available.join("myapp-preflight-1.toml").exists());
        assert!(paths.workers_available.join("myapp-worker-1.toml").exists());
        Ok(())
    }

    #[test]
    fn test_create_container_workers_no_procfile_creates_default_web() -> Result<()> {
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);
        fs::create_dir_all(paths.log_root.join("myapp")).unwrap();
        fs::create_dir_all(paths.env_root.join("myapp")).unwrap();

        let app_path = tmp.path().join("app");
        fs::create_dir_all(&app_path).unwrap();
        // No Procfile — should fall back to default web worker

        let env = HashMap::new();
        create_container_workers("myapp", &app_path, &env, &paths, "docker")?;

        assert!(
            paths.workers_available.join("myapp-web-1.toml").exists(),
            "Default web worker should be created when no Procfile"
        );
        Ok(())
    }
}
