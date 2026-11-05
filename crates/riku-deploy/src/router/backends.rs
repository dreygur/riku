//! The backends an app's router sends traffic to, one per running copy.
//!
//! A Procfile app's copies are the enabled `<app>@<kind>.<n>.toml` worker
//! configs of its web facing kinds; a container app's are the running
//! containers that carry a published port. Both lists are in copy order.
//!
//! Security: worker configs are riku's own files, but the values read from
//! them (port, bind address, socket path) are typed here, and a router
//! renderer still checks what it writes.

use std::collections::HashMap;
use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use riku_container::engine::{ContainerState, Engine};
use riku_container::types::CopyIndex;
use riku_supervisor::config::WorkerConfig;

use crate::config::RikuPaths;
use crate::util::backend::Backend;
use crate::util::file_names::copy_socket;
use crate::workers::web_env::{is_web_facing, is_wsgi_kind};

/// The unix socket the `copy`th uWSGI worker of `app` listens on.
pub fn copy_socket(paths: &RikuPaths, app: &str, copy: CopyIndex) -> PathBuf {
    paths
        .socket_root
        .join(copy_socket::file_name(app, copy.get()))
}

pub(crate) fn parsed_worker(path: &std::path::Path) -> Option<WorkerConfig> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) => {
            tracing::warn!("cannot read worker config {}: {e}", path.display());
            return None;
        }
    };
    match toml::from_str(&text) {
        Ok(config) => Some(config),
        Err(e) => {
            tracing::warn!("cannot parse worker config {}: {e}", path.display());
            None
        }
    }
}

/// The enabled web facing worker configs of `app`, in copy order.
///
/// An unreadable or malformed config belongs to some app's own problem, so it
/// is skipped with a warning instead of failing every other app's routing.
pub(crate) fn web_copies(app: &str, paths: &RikuPaths) -> Result<Vec<WorkerConfig>> {
    let entries = match fs::read_dir(&paths.workers_enabled) {
        Ok(entries) => entries,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => {
            return Err(anyhow!(
                "cannot read {}: {e}",
                paths.workers_enabled.display()
            ))
        }
    };
    let mut copies: Vec<WorkerConfig> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "toml"))
        .filter_map(|path| parsed_worker(&path))
        .filter(|config| config.worker.app == app && is_web_facing(&config.worker.kind))
        .collect();
    copies.sort_by(|a, b| {
        (a.worker.ordinal, &a.worker.kind).cmp(&(b.worker.ordinal, &b.worker.kind))
    });
    Ok(copies)
}

/// Where the router reaches the copy `config` describes: its uWSGI socket for
/// a wsgi kind, else its `PORT` on its bind address.
pub(crate) fn worker_backend(config: &WorkerConfig) -> Result<Backend> {
    if is_wsgi_kind(&config.worker.kind) {
        return match config.env.get("UWSGI_SOCKET") {
            Some(socket) => Ok(Backend::Unix(PathBuf::from(socket))),
            None => Err(anyhow!("{} has no UWSGI_SOCKET", config.worker.name())),
        };
    }
    let port = match config
        .env
        .get("PORT")
        .map(|port| port.trim().parse::<u16>())
    {
        Some(Ok(port)) => port,
        _ => {
            return Err(anyhow!(
                "PORT of {} is not a port number",
                config.worker.name()
            ))
        }
    };
    match crate::nginx::bind_address(&config.env) {
        Ok(host) => Ok(Backend::Tcp { host, port }),
        Err(e) => Err(anyhow!("{}: {e}", config.worker.name())),
    }
}

#[allow(clippy::question_mark)]
fn procfile_backends(app: &str, paths: &RikuPaths) -> Result<Vec<Backend>> {
    let copies = match web_copies(app, paths) {
        Ok(copies) => copies,
        Err(e) => return Err(e),
    };
    copies.iter().map(worker_backend).collect()
}

#[allow(clippy::question_mark)]
fn container_backends(app: &str, engine: &Engine) -> Result<Vec<Backend>> {
    let ids = match engine.list(app) {
        Ok(ids) => ids,
        Err(e) => return Err(e),
    };
    let infos = match engine.inspect(&ids) {
        Ok(infos) => infos,
        Err(e) => return Err(e),
    };
    let mut routed: Vec<(u32, u16)> = infos
        .iter()
        .filter(|info| info.state == ContainerState::Running)
        .filter_map(|info| match (info.copy, info.host_port) {
            (Some(copy), Some(port)) => Some((copy.get(), port)),
            _ => None,
        })
        .collect();
    routed.sort_unstable();
    Ok(routed
        .into_iter()
        .map(|(_, port)| Backend::loopback(port))
        .collect())
}

/// Every backend `app` has right now: one per web copy, in copy order.
///
/// `engine` is `Some` for a container app, whose copies are the running
/// containers with a published port. An app with no copy to route to is an
/// error naming it, so a router is never configured with an empty upstream.
pub fn current_backends(
    app: &str,
    _env: &HashMap<String, String>,
    paths: &RikuPaths,
    engine: Option<&Engine>,
) -> Result<Vec<Backend>> {
    let backends = match engine {
        Some(engine) => container_backends(app, engine),
        None => procfile_backends(app, paths),
    };
    match backends {
        Ok(backends) if backends.is_empty() => {
            Err(anyhow!("app '{app}' has no running web copy to route to"))
        }
        Ok(backends) => Ok(backends),
        Err(e) => Err(anyhow!("cannot find the backends of '{app}': {e}")),
    }
}

#[cfg(test)]
#[path = "backends_tests.rs"]
mod tests;
