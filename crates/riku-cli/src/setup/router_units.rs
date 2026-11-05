//! How riku's own systemd service orders itself after the host router, and
//! the reload watcher units that older versions installed.
//!
//! Reloads now go through the router helper (see [`super::helper_install`]),
//! so init no longer writes a path unit or a reload service. A host that has
//! them from an earlier init gets them disabled and deleted.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use anyhow::{anyhow, Result};

use crate::config::router::Router;

/// The watcher units an earlier init wrote for nginx.
pub const STALE_UNITS: [&str; 2] = ["riku-nginx.path", "riku-nginx-reload.service"];

/// The unit riku's own service orders after: `nginx.service`,
/// `caddy.service`, or empty for a plugin router.
pub fn router_unit_after(router: &Router) -> &'static str {
    match router {
        Router::Nginx => "nginx.service",
        Router::Caddy => "caddy.service",
        Router::Plugin(_) => "",
    }
}

/// The `After=` and `Wants=` lines for a riku service unit.
pub fn unit_ordering(router: &Router) -> String {
    match router_unit_after(router) {
        "" => "After=network.target".to_string(),
        unit => format!("After=network.target {unit}\nWants={unit}"),
    }
}

/// Delete the old watcher unit files from `systemd_dir`. Missing files are
/// fine. Returns whether any existed, so the caller knows to reload systemd.
pub fn remove_stale_units(systemd_dir: &Path) -> Result<bool> {
    let mut removed = false;
    for name in STALE_UNITS {
        let path = systemd_dir.join(name);
        match fs::remove_file(&path) {
            Ok(()) => removed = true,
            Err(e) if e.kind() == ErrorKind::NotFound => {}
            Err(e) => return Err(anyhow!("failed to remove {}: {e}", path.display())),
        }
    }
    Ok(removed)
}

#[cfg(test)]
#[path = "router_units_tests.rs"]
mod tests;
