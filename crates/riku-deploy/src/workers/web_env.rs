//! Environment for web-facing workers: the port or uWSGI socket each one
//! listens on, and the routing keys persisted to the app's ENV file that
//! pick the router's template. The addresses themselves live in each copy's
//! worker config, which `router::current_backends` reads.

use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use riku_container::types::CopyIndex;

use crate::config::RikuPaths;

const UWSGI_PROCESSES: &str = "4";
const UWSGI_THREADS: &str = "4";
const RIKU_NGINX_EXTERNAL_PORT: &str = "80";
const PORT_PICK_ATTEMPTS: usize = 20;

/// Keys older riku versions persisted for the one address of the last copy.
const LEGACY_ADDRESS_KEYS: [&str; 2] = ["RIKU_UPSTREAM_PORT", "UWSGI_SOCKET"];

/// Returns true when `kind` uses a WSGI unix socket (wsgi, jwsgi, rwsgi, php).
pub(crate) fn is_wsgi_kind(kind: &str) -> bool {
    matches!(kind, "wsgi" | "jwsgi" | "rwsgi" | "php")
}

/// Returns true when `kind` needs router wiring (web + all wsgi variants).
pub(crate) fn is_web_facing(kind: &str) -> bool {
    kind == "web" || is_wsgi_kind(kind)
}

/// The unix socket a copy listens on: `RIKU_NGINX_SOCKET` when the app sets
/// one (such an app runs a single copy, see `check_custom_socket`), else a
/// socket of its own per copy.
fn listen_socket(
    app: &str,
    copy: CopyIndex,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
) -> PathBuf {
    match env.get("RIKU_NGINX_SOCKET") {
        Some(socket) => PathBuf::from(socket),
        None => crate::router::copy_socket(paths, app, copy),
    }
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

fn free_loopback_port() -> Result<u16> {
    match crate::util::get_free_port("127.0.0.1") {
        Ok(port) => Ok(port),
        Err(e) => Err(anyhow!("failed to allocate a port: {e}")),
    }
}

/// A port from `pick` that is not in `taken`. The OS hands out a port it
/// has just released again now and then, and two copies on one port leave
/// one of them crash looping behind the router.
#[allow(clippy::question_mark)]
fn unused_port(taken: &[u16], mut pick: impl FnMut() -> Result<u16>) -> Result<u16> {
    for _ in 0..PORT_PICK_ATTEMPTS {
        let port = match pick() {
            Ok(port) => port,
            Err(e) => return Err(e),
        };
        if !taken.contains(&port) {
            return Ok(port);
        }
    }
    Err(anyhow!(
        "could not find a port distinct from the app's other copies"
    ))
}

/// Use the pinned port or allocate a free one, and inject nginx port-map
/// variables into the environment.
#[allow(clippy::question_mark)]
fn configure_web_env(
    socket_path: &Path,
    env: &mut HashMap<String, String>,
    taken: &[u16],
    pick: impl FnMut() -> Result<u16>,
) -> Result<()> {
    let port = match pinned_port(env) {
        Ok(Some(port)) => port,
        Ok(None) => match unused_port(taken, pick) {
            Ok(port) => port,
            Err(e) => return Err(e),
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
///
/// Neither mode persists a port or socket: with several copies there is no
/// single value to keep, and the router reads each copy's address from its
/// worker config. The old single address keys are dropped when found.
fn apply_route_keys(kind: &str, persisted: &mut HashMap<String, String>) {
    for key in LEGACY_ADDRESS_KEYS {
        persisted.remove(key);
    }
    if is_wsgi_kind(kind) {
        persisted.remove("RIKU_NGINX_PORTMAP");
        persisted.remove("RIKU_NGINX_EXTERNAL_PORT");
        persisted.insert("RIKU_NGINX_WSGI".to_string(), "true".to_string());
        return;
    }

    persisted.remove("RIKU_NGINX_WSGI");
    persisted.insert("RIKU_NGINX_PORTMAP".to_string(), "true".to_string());
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
fn persist_route_env(app: &str, kind: &str, paths: &RikuPaths) -> Result<()> {
    let env_dir = paths.env_root.join(app);
    if let Err(e) = fs::create_dir_all(&env_dir) {
        return Err(anyhow!("failed to create {}: {e}", env_dir.display()));
    }
    let env_file = env_dir.join("ENV");

    let mut persisted: HashMap<String, String> = HashMap::new();
    if let Err(e) = crate::util::parse_settings(&env_file, &mut persisted) {
        return Err(anyhow!("failed to read {}: {e}", env_file.display()));
    }

    apply_route_keys(kind, &mut persisted);

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
    copy: CopyIndex,
    base_env: &HashMap<String, String>,
    taken: &[u16],
    paths: &RikuPaths,
) -> Result<HashMap<String, String>> {
    let mut env = base_env.clone();

    if !is_web_facing(kind) {
        return Ok(env);
    }

    let socket_path = listen_socket(app, copy, base_env, paths);

    if is_wsgi_kind(kind) {
        configure_wsgi_env(&socket_path, &mut env);
    } else if let Err(e) = configure_web_env(&socket_path, &mut env, taken, free_loopback_port) {
        return Err(e);
    }

    if let Err(e) = persist_route_env(app, kind, paths) {
        return Err(e);
    }

    Ok(env)
}

#[cfg(test)]
#[path = "web_env_tests.rs"]
mod tests;
