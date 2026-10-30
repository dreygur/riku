//! Checks for an app that pins its web port with `PORT` in its env.
//!
//! They run before any worker config changes, so a refused deploy leaves the
//! running app alone. Without them the router would point the app's domain
//! at whatever already holds the port, serving another app's pages.

use std::collections::HashMap;
use std::fs;
use std::io::ErrorKind;
use std::net::TcpListener;

use anyhow::{anyhow, Result};

use crate::config::RikuPaths;

use super::web_env::pinned_port;
use super::{read_scaling_count, WEB_KIND};

/// Env keys that say which port an app is served on.
const PORT_KEYS: [&str; 2] = ["PORT", "RIKU_UPSTREAM_PORT"];

/// Another riku app that pins `port` or is routed to it.
fn app_on_port(port: u16, app: &str, paths: &RikuPaths) -> Option<String> {
    let entries = fs::read_dir(&paths.env_root).ok()?;
    let wanted = port.to_string();
    entries.flatten().find_map(|entry| {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == app {
            return None;
        }
        let mut env = HashMap::new();
        if crate::util::parse_settings(&entry.path().join("ENV"), &mut env).is_err() {
            return None;
        }
        PORT_KEYS
            .iter()
            .any(|key| env.get(*key) == Some(&wanted))
            .then_some(name)
    })
}

/// Whether some process listens on `port`. Only `AddrInUse` counts: binding
/// a port below 1024 as a normal user fails for another reason.
fn port_in_use(port: u16) -> bool {
    match TcpListener::bind(("127.0.0.1", port)) {
        Ok(_) => false,
        Err(e) => e.kind() == ErrorKind::AddrInUse,
    }
}

/// Refuse a pinned port that is scaled past one process, used by another
/// app, or held by a process that is not this app's previous deploy.
#[allow(clippy::question_mark)]
pub(super) fn check_pinned_port(
    app: &str,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
) -> Result<()> {
    let port = match pinned_port(env) {
        Ok(Some(port)) => port,
        Ok(None) => return Ok(()),
        Err(e) => return Err(e),
    };

    let count = match read_scaling_count(paths, app, WEB_KIND) {
        Ok(count) => count,
        Err(e) => return Err(e),
    };
    if count > 1 {
        return Err(anyhow!(
            "web is scaled to {count} but PORT is pinned to {port}; a pinned port runs one web process"
        ));
    }

    if let Some(other) = app_on_port(port, app, paths) {
        return Err(anyhow!("PORT {port} is already used by app '{other}'"));
    }

    let ours = env.get("RIKU_UPSTREAM_PORT") == Some(&port.to_string());
    if !ours && port_in_use(port) {
        return Err(anyhow!(
            "PORT {port} is in use by another process on this host"
        ));
    }
    Ok(())
}
