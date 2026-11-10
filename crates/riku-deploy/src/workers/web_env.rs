//! Environment for web-facing workers: the port or uWSGI socket each one
//! listens on, and the routing keys persisted to the app's ENV file that
//! pick the router's template. The addresses themselves live in each copy's
//! worker config, which `router::current_backends` reads.

use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use riku_container::types::CopyIndex;

use super::ports::{free_loopback_port, pinned_port, unused_port};
use crate::config::RikuPaths;

const UWSGI_PROCESSES: &str = "4";
const UWSGI_THREADS: &str = "4";
/// The nginx only listen port, replaced by riku.toml `routing.port`; dropped
/// wherever it is found.
const REMOVED_LISTEN_KEY: &str = "RIKU_NGINX_EXTERNAL_PORT";

/// Keys older riku versions persisted for the one address of the last copy.
const LEGACY_ADDRESS_KEYS: [&str; 3] = ["RIKU_UPSTREAM_PORT", "UWSGI_SOCKET", REMOVED_LISTEN_KEY];
/// The keys a web facing kind persists to pick the router's template.
const ROUTE_KEYS: [&str; 2] = ["RIKU_NGINX_PORTMAP", "RIKU_NGINX_WSGI"];

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

/// The socket directory must exist before uWSGI binds in it. Root init makes
/// it; this only matters after a reboot on a host without systemd-tmpfiles,
/// and in tests. A `RIKU_NGINX_SOCKET` lives wherever the app put it.
fn ensure_socket_root(env: &HashMap<String, String>, paths: &RikuPaths) -> Result<()> {
    if env.contains_key("RIKU_NGINX_SOCKET") {
        return Ok(());
    }
    crate::util::socket_dir::ensure_exists(&paths.socket_root, crate::config::SOCKET_GROUP)
}

/// Inject uwsgi unix-socket variables into the environment.
///
/// uWSGI reads `UWSGI_<OPTION>` variables as its own options. The socket is
/// `0660` in the setgid `riku-sockets` directory, so it belongs to that
/// group, which root init puts the nginx worker user in, and is never world
/// writable. `die-on-term` makes
/// SIGTERM stop uWSGI instead of reloading it, so a stopped copy exits
/// at once rather than after the supervisor's SIGKILL timeout.
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
    env.insert("UWSGI_CHMOD_SOCKET".to_string(), "660".to_string());
    env.insert("UWSGI_DIE_ON_TERM".to_string(), "1".to_string());
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
    env.remove(REMOVED_LISTEN_KEY);
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
        persisted.insert("RIKU_NGINX_WSGI".to_string(), "true".to_string());
        return;
    }

    persisted.remove("RIKU_NGINX_WSGI");
    persisted.insert("RIKU_NGINX_PORTMAP".to_string(), "true".to_string());
}

/// Drop every routing key, for an app with no web facing kind left.
/// Returns whether any key was there.
pub(crate) fn drop_route_keys(persisted: &mut HashMap<String, String>) -> bool {
    let before = persisted.len();
    for key in ROUTE_KEYS.iter().chain(LEGACY_ADDRESS_KEYS.iter()) {
        persisted.remove(*key);
    }
    persisted.len() != before
}

/// Read the app's ENV file, let `change` edit it, and write it back when
/// `change` returns true.
#[allow(clippy::question_mark)]
fn rewrite_app_env(
    app: &str,
    paths: &RikuPaths,
    change: impl FnOnce(&mut HashMap<String, String>) -> bool,
) -> Result<()> {
    let env_dir = paths.env_root.join(app);
    if let Err(e) = crate::util::create_private_dir(&env_dir) {
        return Err(e);
    }
    let env_file = env_dir.join("ENV");

    let mut persisted: HashMap<String, String> = HashMap::new();
    if let Err(e) = crate::util::parse_settings(&env_file, &mut persisted) {
        return Err(anyhow!("failed to read {}: {e}", env_file.display()));
    }
    if !change(&mut persisted) {
        return Ok(());
    }
    match crate::util::write_config(&env_file, &persisted, "=") {
        Ok(()) => Ok(()),
        Err(e) => Err(anyhow!("failed to write {}: {e}", env_file.display())),
    }
}

/// Set the app ENV file's routing keys for `kind`, on every deploy, so the
/// template the router picks follows the kind the Procfile has now. The
/// addresses are not persisted, see `apply_route_keys`.
fn persist_route_env(app: &str, kind: &str, paths: &RikuPaths) -> Result<()> {
    rewrite_app_env(app, paths, |persisted| {
        apply_route_keys(kind, persisted);
        true
    })
}

/// Remove the routing keys from the app's ENV file. A deploy with no web
/// facing kind calls this, so a port map left by an old `web:` line cannot
/// win over `RIKU_NGINX_STATIC` or proxy to a port nothing of the app uses.
pub(crate) fn clear_route_env(app: &str, paths: &RikuPaths) -> Result<()> {
    rewrite_app_env(app, paths, drop_route_keys)
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
        if let Err(e) = ensure_socket_root(base_env, paths) {
            return Err(e);
        }
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
