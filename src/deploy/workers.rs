//! Generic worker configuration creation for deployed apps.
//!
//! Handles Procfile parsing and worker config generation.
//! Scaling delta logic lives in `super::scaling`.

use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::config::RikuPaths;
use crate::util::echo;

pub(crate) use super::scaling::apply_scaling_deltas;

/// Read the scaling count for a given process kind from the SCALING file.
///
/// Returns 1 if the file doesn't exist or the kind isn't listed.
pub fn read_scaling_count(paths: &RikuPaths, app: &str, kind: &str) -> Result<u32> {
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
            let key = line[..pos].trim();
            let val = line[pos + 1..].trim();
            if key == kind {
                if let Ok(n) = val.parse::<u32>() {
                    return Ok(n);
                }
            }
        }
    }
    Ok(1)
}

/// Generic worker configuration creation for standard runtimes.
/// This eliminates ~60 lines of duplicated code per runtime.
///
/// Runtimes with special requirements (Python venv, Node version, etc.) can still
/// use their custom implementations.
pub fn create_workers_generic(
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
) -> Result<()> {
    use crate::supervisor::config::create_worker_config;
    use crate::util::get_free_port;

    // Handle RIKU_AUTO_RESTART - if false, skip removing existing worker configs
    let auto_restart = env
        .get("RIKU_AUTO_RESTART")
        .map(|v| v.to_lowercase() != "false" && v != "0" && v != "no")
        .unwrap_or(true);

    if auto_restart {
        // Remove existing worker configs to trigger restart
        for ext in &["toml", "ini"] {
            // Use "{app}-*" not "{app}*" to avoid matching configs for apps
            // whose names share a prefix (e.g. "foo" would otherwise delete
            // configs for "foobar").
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
                                break;
                            }
                        }
                    }
                }
            }

            // Create worker configs for each instance
            for i in 1..=count {
                // Prepare environment for the worker
                let mut worker_env = env.clone();

                // Set PORT/WSGI_SOCKET for web/wsgi/jwsgi/rwsgi/php processes
                // wsgi/jwsgi/rwsgi use unix socket, others use TCP port
                let final_command = if kind == "web"
                    || kind == "wsgi"
                    || kind == "jwsgi"
                    || kind == "rwsgi"
                    || kind == "php"
                {
                    // Create socket file for wsgi/jwsgi/rwsgi/php (unix socket)
                    // For plain web, we use TCP port (NGINX_PORTMAP)
                    let socket_path = paths.nginx_root.join(format!("{}.sock", app));

                    if kind == "wsgi" || kind == "jwsgi" || kind == "rwsgi" || kind == "php" {
                        // Use unix socket with uwsgi protocol
                        worker_env.insert(
                            "SOCKET".to_string(),
                            format!("unix://{}", socket_path.to_string_lossy()),
                        );
                        worker_env.insert(
                            "UWSGI_SOCKET".to_string(),
                            socket_path.to_string_lossy().to_string(),
                        );
                        worker_env.insert("NGINX_WSGI".to_string(), "true".to_string());

                        // Add uwsgi-specific env vars
                        worker_env.insert("UWSGI_PROCESSES".to_string(), "4".to_string());
                        worker_env.insert("UWSGI_THREADS".to_string(), "4".to_string());
                    } else {
                        // Plain web uses TCP port
                        let port = get_free_port("127.0.0.1")?;
                        worker_env.insert("PORT".to_string(), port.to_string());
                        worker_env.insert("NGINX_PORTMAP".to_string(), "true".to_string());
                        worker_env.insert("NGINX_INTERNAL_PORT".to_string(), port.to_string());
                        worker_env.insert("NGINX_EXTERNAL_PORT".to_string(), "80".to_string());
                    }

                    // For plain web workers, set the SOCKET env var to the socket path.
                    // wsgi/jwsgi/rwsgi/php workers already set SOCKET above with the
                    // uwsgi unix:// prefix, so do not overwrite it here.
                    if kind != "wsgi" && kind != "jwsgi" && kind != "rwsgi" && kind != "php" {
                        worker_env.insert(
                            "SOCKET".to_string(),
                            socket_path.to_string_lossy().to_string(),
                        );
                    }

                    // Write NGINX settings to ENV file
                    let env_dir = paths.env_root.join(app);
                    fs::create_dir_all(&env_dir)?;
                    let env_file = env_dir.join("ENV");

                    let mut env_content = if env_file.exists() {
                        fs::read_to_string(&env_file)?
                    } else {
                        String::new()
                    };

                    if !env_content.contains("NGINX_PORTMAP") && !env_content.contains("NGINX_WSGI")
                    {
                        if kind == "wsgi" || kind == "jwsgi" || kind == "rwsgi" || kind == "php" {
                            env_content.push_str("NGINX_WSGI=true\n");
                            env_content
                                .push_str(&format!("UWSGI_SOCKET={}\n", socket_path.display()));
                        } else {
                            let port = worker_env.get("PORT").map(|s| s.as_str()).unwrap_or("8080");
                            env_content.push_str("NGINX_PORTMAP=true\n");
                            env_content.push_str(&format!("NGINX_INTERNAL_PORT={}\n", port));
                            env_content.push_str("NGINX_EXTERNAL_PORT=80\n");
                        }
                        fs::write(&env_file, &env_content)?;
                    }

                    command.to_string()
                } else {
                    command.to_string()
                };

                // Create the worker config
                let worker_config = create_worker_config(
                    app,
                    kind,
                    &final_command,
                    i,
                    worker_env,
                    &app_path.to_string_lossy(),
                    &paths
                        .log_root
                        .join(app)
                        .join(format!("{}.{}.log", kind, i))
                        .to_string_lossy(),
                );

                // Write the worker config to the available directory
                let config_filename = format!("{}-{}-{}.toml", app, kind, i);
                let config_path = paths.workers_available.join(&config_filename);

                let config_content = toml::to_string(&worker_config)?;
                fs::write(&config_path, &config_content)?;

                // Create a symlink to enable the worker
                let enabled_path = paths.workers_enabled.join(&config_filename);
                if enabled_path.exists() {
                    fs::remove_file(&enabled_path)?;
                }
                std::os::unix::fs::symlink(&config_path, &enabled_path)?;

                echo(
                    &format!("-----> Created worker config: {}", config_filename),
                    "green",
                );
            }
        }
    }

    Ok(())
}

