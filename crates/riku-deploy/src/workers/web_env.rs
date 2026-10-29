//! Environment for web-facing workers: the port or uWSGI socket each one
//! listens on, and the routing keys persisted to the app's ENV file so
//! `spawn_app` can point the router at it.

use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::config::RikuPaths;

const UWSGI_PROCESSES: &str = "4";
const UWSGI_THREADS: &str = "4";
const RIKU_NGINX_EXTERNAL_PORT: &str = "80";

/// Keys persisted for a uWSGI socket app.
const WSGI_KEYS: [&str; 2] = ["RIKU_NGINX_WSGI", "UWSGI_SOCKET"];
/// Keys persisted for a TCP port app.
const PORTMAP_KEYS: [&str; 3] = [
    "RIKU_NGINX_PORTMAP",
    "RIKU_UPSTREAM_PORT",
    "RIKU_NGINX_EXTERNAL_PORT",
];

/// Returns true when `kind` uses a WSGI unix socket (wsgi, jwsgi, rwsgi, php).
fn is_wsgi_kind(kind: &str) -> bool {
    matches!(kind, "wsgi" | "jwsgi" | "rwsgi" | "php")
}

/// Returns true when `kind` needs router wiring (web + all wsgi variants).
fn is_web_facing(kind: &str) -> bool {
    kind == "web" || is_wsgi_kind(kind)
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
    env.insert("RIKU_NGINX_WSGI".to_string(), "true".to_string());
    env.insert("UWSGI_PROCESSES".to_string(), UWSGI_PROCESSES.to_string());
    env.insert("UWSGI_THREADS".to_string(), UWSGI_THREADS.to_string());
}

/// The `PORT` the app's ENV pins, if any. Some apps need a fixed port, such
/// as a compose file that publishes one.
pub(super) fn pinned_port(env: &HashMap<String, String>) -> Result<Option<u16>> {
    let Some(value) = env.get("PORT") else {
        return Ok(None);
    };
    match value.parse::<u16>() {
        Ok(port) if port != 0 => Ok(Some(port)),
        _ => Err(anyhow!("PORT in the app env is not a port: {value:?}")),
    }
}

/// Use the pinned port or allocate a free one, and inject nginx port-map
/// variables into the environment.
fn configure_web_env(socket_path: &Path, env: &mut HashMap<String, String>) -> Result<()> {
    let port = match pinned_port(env) {
        Ok(Some(port)) => port,
        Ok(None) => match crate::util::get_free_port("127.0.0.1") {
            Ok(port) => port,
            Err(e) => return Err(anyhow!("failed to allocate a port: {e}")),
        },
        Err(e) => return Err(e),
    };
    env.insert("PORT".to_string(), port.to_string());
    env.insert("RIKU_NGINX_PORTMAP".to_string(), "true".to_string());
    env.insert("RIKU_UPSTREAM_PORT".to_string(), port.to_string());
    env.insert(
        "RIKU_NGINX_EXTERNAL_PORT".to_string(),
        RIKU_NGINX_EXTERNAL_PORT.to_string(),
    );
    env.insert(
        "SOCKET".to_string(),
        socket_path.to_string_lossy().to_string(),
    );
    Ok(())
}

/// Set the routing keys for `kind` and drop the other mode's keys.
///
/// An app that moves from `wsgi:` to `web:` (or back) would otherwise keep
/// the old mode's keys: nginx picks its wsgi template whenever
/// `RIKU_NGINX_WSGI` is present, and Caddy rejects the app outright.
fn apply_route_keys(
    kind: &str,
    socket_path: &Path,
    env: &HashMap<String, String>,
    persisted: &mut HashMap<String, String>,
) {
    if is_wsgi_kind(kind) {
        for key in PORTMAP_KEYS {
            persisted.remove(key);
        }
        persisted.insert("RIKU_NGINX_WSGI".to_string(), "true".to_string());
        persisted.insert(
            "UWSGI_SOCKET".to_string(),
            socket_path.to_string_lossy().to_string(),
        );
        return;
    }

    for key in WSGI_KEYS {
        persisted.remove(key);
    }
    let port = env.get("PORT").map(|s| s.as_str()).unwrap_or("8080");
    persisted.insert("RIKU_NGINX_PORTMAP".to_string(), "true".to_string());
    persisted.insert("RIKU_UPSTREAM_PORT".to_string(), port.to_string());
    persisted.insert(
        "RIKU_NGINX_EXTERNAL_PORT".to_string(),
        RIKU_NGINX_EXTERNAL_PORT.to_string(),
    );
}

/// Write routing variables to the app's ENV file, refreshing them on
/// every deploy.
///
/// `configure_web_env`/`configure_wsgi_env` allocate a fresh ephemeral port
/// (or socket) on every single deploy, not just the first, so the values
/// persisted here (read back by `spawn_app`'s router configure call)
/// must always be overwritten to match, never just "written once". A
/// previous version of this function skipped the whole write whenever the
/// ENV file already contained `RIKU_NGINX_PORTMAP`/`RIKU_NGINX_WSGI` from an earlier
/// deploy, which left `RIKU_UPSTREAM_PORT` pinned to that first deploy's
/// port forever: every redeploy after the first would spawn the worker on
/// a new port while nginx kept proxying to the old, now-dead one, a
/// guaranteed 502 on the very next deploy of any web app.
fn persist_route_env(
    app: &str,
    kind: &str,
    socket_path: &Path,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
) -> Result<()> {
    let env_dir = paths.env_root.join(app);
    if let Err(e) = fs::create_dir_all(&env_dir) {
        return Err(anyhow!("failed to create {}: {e}", env_dir.display()));
    }
    let env_file = env_dir.join("ENV");

    let mut persisted: HashMap<String, String> = HashMap::new();
    if let Err(e) = crate::util::parse_settings(&env_file, &mut persisted) {
        return Err(anyhow!("failed to read {}: {e}", env_file.display()));
    }

    apply_route_keys(kind, socket_path, env, &mut persisted);

    match crate::util::write_config(&env_file, &persisted, "=") {
        Ok(()) => Ok(()),
        Err(e) => Err(anyhow!("failed to write {}: {e}", env_file.display())),
    }
}

/// Build the environment map for a single worker instance.
///
/// Web-facing processes get PORT/SOCKET injected; wsgi variants get uwsgi vars.
/// Also persists routing settings back to the app's ENV file.
#[allow(clippy::question_mark)]
pub(super) fn build_worker_env(
    app: &str,
    kind: &str,
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
    } else if let Err(e) = configure_web_env(&socket_path, &mut env) {
        return Err(e);
    }

    if let Err(e) = persist_route_env(app, kind, &socket_path, &env, paths) {
        return Err(e);
    }

    Ok(env)
}

#[cfg(test)]
#[path = "web_env_tests.rs"]
mod tests;
