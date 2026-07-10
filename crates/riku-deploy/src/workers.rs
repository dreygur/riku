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

const UWSGI_PROCESSES: &str = "4";
const UWSGI_THREADS: &str = "4";
const NGINX_EXTERNAL_PORT: &str = "80";

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

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

/// Create worker configs for every process entry in the app's Procfile.
///
/// `start_cmd` is an optional fallback command supplied by the runtime plugin
/// via its `start` subcommand. It is used only when a Procfile entry has an
/// empty command.
pub fn create_workers_generic(
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
    start_cmd: Option<&str>,
) -> Result<()> {
    if should_restart(env) {
        remove_stale_configs(app, paths);
    }

    let entries = match parse_procfile(app_path)? {
        Some(e) => e,
        None => return Ok(()),
    };

    for (kind, command) in &entries {
        let effective_cmd = if command.is_empty() {
            start_cmd.unwrap_or(command.as_str())
        } else {
            command.as_str()
        };
        let count = read_scaling_count(paths, app, kind)?;
        for i in 1..=count {
            let worker_env = build_worker_env(app, kind, effective_cmd, env, paths)?;
            write_worker_config(app, app_path, kind, effective_cmd, i, worker_env, paths)?;
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Returns true when `RIKU_AUTO_RESTART` is not explicitly disabled.
fn should_restart(env: &HashMap<String, String>) -> bool {
    env.get("RIKU_AUTO_RESTART")
        .map(|v| v.to_lowercase() != "false" && v != "0" && v != "no")
        .unwrap_or(true)
}

/// Remove existing worker symlinks from `workers_enabled` to trigger a restart.
///
/// Uses `"{app}-*"` not `"{app}*"` to avoid touching configs for apps
/// whose names share a prefix (e.g. "foo" would otherwise match "foobar").
fn remove_stale_configs(app: &str, paths: &RikuPaths) {
    for ext in &["toml", "ini"] {
        let pattern = paths.workers_enabled.join(format!("{}-*.{}", app, ext));
        if let Ok(entries) = glob::glob(pattern.to_str().unwrap_or("")) {
            for entry in entries.flatten() {
                if let Err(e) = fs::remove_file(&entry) {
                    tracing::warn!("Could not remove stale worker config {:?}: {}", entry, e);
                }
            }
        }
    }
}

/// Parse the Procfile at `app_path/Procfile` into `(kind, command)` pairs.
///
/// Returns `None` (and prints a warning) when no Procfile is found.
/// Comment lines and blank lines are skipped.
fn parse_procfile(app_path: &Path) -> Result<Option<Vec<(String, String)>>> {
    let procfile_path = app_path.join("Procfile");
    if !procfile_path.exists() {
        echo(
            "-----> No Procfile found, skipping process creation",
            "yellow",
        );
        return Ok(None);
    }

    let content = fs::read_to_string(&procfile_path)?;
    let entries = content.lines().filter_map(parse_procfile_line).collect();

    Ok(Some(entries))
}

/// Parse a single Procfile line into `(kind, command)`, or `None` for blank/comment lines.
fn parse_procfile_line(line: &str) -> Option<(String, String)> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let pos = line.find(':')?;
    let kind = line[..pos].trim().to_string();
    let command = line[pos + 1..].trim().to_string();
    Some((kind, command))
}

/// Returns true when `kind` uses a WSGI unix socket (wsgi, jwsgi, rwsgi, php).
fn is_wsgi_kind(kind: &str) -> bool {
    matches!(kind, "wsgi" | "jwsgi" | "rwsgi" | "php")
}

/// Returns true when `kind` needs nginx wiring (web + all wsgi variants).
fn is_web_facing(kind: &str) -> bool {
    kind == "web" || is_wsgi_kind(kind)
}

/// Build the environment map for a single worker instance.
///
/// Web-facing processes get PORT/SOCKET injected; wsgi variants get uwsgi vars.
/// Also persists nginx settings back to the app's ENV file.
fn build_worker_env(
    app: &str,
    kind: &str,
    command: &str,
    base_env: &HashMap<String, String>,
    paths: &RikuPaths,
) -> Result<HashMap<String, String>> {
    let mut env = base_env.clone();

    if !is_web_facing(kind) {
        return Ok(env);
    }

    let socket_path = paths.nginx_root.join(format!("{}.sock", app));

    if is_wsgi_kind(kind) {
        configure_wsgi_env(&socket_path, &mut env);
    } else {
        configure_web_env(&socket_path, &mut env, paths)?;
    }

    persist_nginx_env(app, kind, command, &socket_path, &env, paths)?;

    Ok(env)
}

/// Inject uwsgi unix-socket variables into the environment.
fn configure_wsgi_env(socket_path: &Path, env: &mut HashMap<String, String>) {
    env.insert(
        "SOCKET".to_string(),
        format!("unix://{}", socket_path.to_string_lossy()),
    );
    env.insert(
        "UWSGI_SOCKET".to_string(),
        socket_path.to_string_lossy().to_string(),
    );
    env.insert("NGINX_WSGI".to_string(), "true".to_string());
    env.insert("UWSGI_PROCESSES".to_string(), UWSGI_PROCESSES.to_string());
    env.insert("UWSGI_THREADS".to_string(), UWSGI_THREADS.to_string());
}

/// Allocate a free TCP port and inject nginx port-map variables into the environment.
fn configure_web_env(
    socket_path: &Path,
    env: &mut HashMap<String, String>,
    paths: &RikuPaths,
) -> Result<()> {
    use crate::util::get_free_port;

    let port = get_free_port("127.0.0.1")?;
    env.insert("PORT".to_string(), port.to_string());
    env.insert("NGINX_PORTMAP".to_string(), "true".to_string());
    env.insert("NGINX_INTERNAL_PORT".to_string(), port.to_string());
    env.insert(
        "NGINX_EXTERNAL_PORT".to_string(),
        NGINX_EXTERNAL_PORT.to_string(),
    );
    env.insert(
        "SOCKET".to_string(),
        socket_path.to_string_lossy().to_string(),
    );

    // Suppress unused warning — paths is used by callers for socket_path resolution
    let _ = paths;

    Ok(())
}

/// Write nginx-related variables to the app's ENV file, refreshing them on
/// every deploy.
///
/// `configure_web_env`/`configure_wsgi_env` allocate a fresh ephemeral port
/// (or socket) on every single deploy, not just the first — so the values
/// persisted here (read back by `spawn_app`'s `generate_nginx_config` call)
/// must always be overwritten to match, never just "written once". A
/// previous version of this function skipped the whole write whenever the
/// ENV file already contained `NGINX_PORTMAP`/`NGINX_WSGI` from an earlier
/// deploy, which left `NGINX_INTERNAL_PORT` pinned to that first deploy's
/// port forever: every redeploy after the first would spawn the worker on
/// a new port while nginx kept proxying to the old, now-dead one — a
/// guaranteed 502 on the very next deploy of any web app.
fn persist_nginx_env(
    app: &str,
    kind: &str,
    _command: &str,
    socket_path: &Path,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
) -> Result<()> {
    let env_dir = paths.env_root.join(app);
    fs::create_dir_all(&env_dir)?;
    let env_file = env_dir.join("ENV");

    let mut persisted: HashMap<String, String> = HashMap::new();
    crate::util::parse_settings(&env_file, &mut persisted)?;

    if is_wsgi_kind(kind) {
        persisted.insert("NGINX_WSGI".to_string(), "true".to_string());
        persisted.insert(
            "UWSGI_SOCKET".to_string(),
            socket_path.to_string_lossy().to_string(),
        );
    } else {
        let port = env.get("PORT").map(|s| s.as_str()).unwrap_or("8080");
        persisted.insert("NGINX_PORTMAP".to_string(), "true".to_string());
        persisted.insert("NGINX_INTERNAL_PORT".to_string(), port.to_string());
        persisted.insert(
            "NGINX_EXTERNAL_PORT".to_string(),
            NGINX_EXTERNAL_PORT.to_string(),
        );
    }

    crate::util::write_config(&env_file, &persisted, "=")?;
    Ok(())
}

/// Write a single worker TOML config to `workers_available` and symlink it into `workers_enabled`.
fn write_worker_config(
    app: &str,
    app_path: &Path,
    kind: &str,
    command: &str,
    index: u32,
    env: HashMap<String, String>,
    paths: &RikuPaths,
) -> Result<()> {
    use crate::supervisor::config::create_worker_config;

    let log_path = paths
        .log_root
        .join(app)
        .join(format!("{}.{}.log", kind, index));

    let config = create_worker_config(
        app,
        kind,
        command,
        index,
        env,
        &app_path.to_string_lossy(),
        &log_path.to_string_lossy(),
    );

    let filename = format!("{}-{}-{}.toml", app, kind, index);
    let available = paths.workers_available.join(&filename);
    let enabled = paths.workers_enabled.join(&filename);

    crate::util::write_atomic(&available, toml::to_string(&config)?.as_bytes())?;

    // Atomic symlink swap: create the new symlink under a temp name, then
    // `rename` it over `enabled`. A crash or concurrent reader between a
    // `remove_file` and a fresh `symlink` would otherwise see `enabled`
    // briefly missing entirely; `rename` guarantees the path always
    // resolves to either the old or the new symlink, never nothing.
    let tmp_link = paths
        .workers_enabled
        .join(format!(".{}.tmp-{}", filename, std::process::id()));
    let _ = fs::remove_file(&tmp_link);
    std::os::unix::fs::symlink(&available, &tmp_link)?;
    fs::rename(&tmp_link, &enabled)?;

    echo(
        &format!("-----> Created worker config: {}", filename),
        "green",
    );
    Ok(())
}

#[cfg(test)]
#[path = "workers_tests.rs"]
mod tests;
