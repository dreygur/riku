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

use super::ports::pinned_port;
use super::{read_scaling_count, WEB_KIND};

/// Whether one of `app`'s enabled web copies runs on `port`.
fn copy_on_port(port: u16, app: &str, paths: &RikuPaths) -> Result<bool> {
    let wanted = port.to_string();
    match crate::router::web_copies(app, paths) {
        Ok(copies) => Ok(copies
            .iter()
            .any(|copy| copy.env.get("PORT") == Some(&wanted))),
        Err(e) => Err(anyhow!("cannot read the web copies of '{app}': {e}")),
    }
}

/// Whether the app named `name` pins `port` in its ENV or runs a copy on it.
/// A `RIKU_UPSTREAM_PORT` left in an old ENV file says nothing about a live
/// app, so only `PORT` and the enabled copies count.
fn app_uses_port(port: u16, name: &str, paths: &RikuPaths) -> Result<bool> {
    let mut env = HashMap::new();
    let env_file = paths.env_root.join(name).join("ENV");
    if crate::util::parse_settings(&env_file, &mut env).is_ok()
        && env.get("PORT") == Some(&port.to_string())
    {
        return Ok(true);
    }
    copy_on_port(port, name, paths)
}

/// Another riku app that pins `port` or is routed to it.
fn app_on_port(port: u16, app: &str, paths: &RikuPaths) -> Result<Option<String>> {
    let entries = match fs::read_dir(&paths.env_root) {
        Ok(entries) => entries,
        Err(_) => return Ok(None),
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == app {
            continue;
        }
        match app_uses_port(port, &name, paths) {
            Ok(true) => return Ok(Some(name)),
            Ok(false) => {}
            Err(e) => return Err(e),
        }
    }
    Ok(None)
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
pub(super) fn check_pinned_port(
    app: &str,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
) -> Result<()> {
    check_pinned_port_with(app, env, paths, port_in_use)
}

/// [`check_pinned_port`] with the probe for a busy port injected, so a test
/// of the claim logic does not depend on which ports the host has free.
#[allow(clippy::question_mark)]
pub(super) fn check_pinned_port_with(
    app: &str,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
    in_use: impl Fn(u16) -> bool,
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

    match app_on_port(port, app, paths) {
        Ok(Some(other)) => return Err(anyhow!("PORT {port} is already used by app '{other}'")),
        Ok(None) => {}
        Err(e) => return Err(e),
    }

    let ours = match copy_on_port(port, app, paths) {
        Ok(ours) => ours,
        Err(e) => return Err(e),
    };
    if !ours && in_use(port) {
        return Err(anyhow!(
            "PORT {port} is in use by another process on this host"
        ));
    }
    Ok(())
}
