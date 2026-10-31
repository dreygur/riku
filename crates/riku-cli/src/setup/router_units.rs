//! systemd units that reload the host router when riku changes its config.
//!
//! nginx gets a path unit watching its config directory under the deploy
//! user's riku root, plus a oneshot service that runs `systemctl reload`
//! on it. Caddy and router plugins get no units: riku reloads them itself
//! and waits for the result. Units written for Caddy by older versions are
//! removed.

use std::fs;
use std::path::Path;

use anyhow::{anyhow, Result};

use crate::config::router::Router;

const SYSTEMCTL: &str = "/usr/bin/systemctl";
const NGINX_SERVICE: &str = "nginx";
const CADDY_SERVICE: &str = "caddy";

/// The systemd service of a built-in router, `None` for a plugin.
fn router_service(router: &Router) -> Option<&'static str> {
    match router {
        Router::Nginx => Some(NGINX_SERVICE),
        Router::Caddy | Router::Plugin(_) => None,
    }
}

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

fn path_unit_name(service: &str) -> String {
    format!("riku-{service}.path")
}

fn reload_unit_name(service: &str) -> String {
    format!("riku-{service}-reload.service")
}

fn render_path_unit(service: &str, watched: &Path) -> String {
    format!(
        "[Unit]
Description=Watch for Riku {service} configuration changes
Documentation=https://dreygur.github.io/riku/
PartOf=riku.service

[Path]
PathModified={watched}
Unit={reload}

[Install]
WantedBy=multi-user.target
",
        watched = watched.display(),
        reload = reload_unit_name(service),
    )
}

fn render_reload_unit(service: &str) -> String {
    format!(
        "[Unit]
Description=Reload {service} when Riku configuration changes
Documentation=https://dreygur.github.io/riku/

[Service]
Type=oneshot
ExecStart={SYSTEMCTL} reload {service}

[Install]
WantedBy=multi-user.target
"
    )
}

fn write_unit(systemd_dir: &Path, name: &str, content: &str) -> Result<()> {
    let path = systemd_dir.join(name);
    match fs::write(&path, content) {
        Ok(()) => Ok(()),
        Err(e) => Err(anyhow!("failed to write {}: {e}", path.display())),
    }
}

/// Write the watcher units for `router`, watching `config_dir`. Returns the
/// path unit to enable, `None` for a plugin router.
#[allow(clippy::question_mark)]
pub fn write_router_units(
    systemd_dir: &Path,
    router: &Router,
    config_dir: &Path,
) -> Result<Option<String>> {
    let Some(service) = router_service(router) else {
        return Ok(None);
    };
    let path_unit = path_unit_name(service);
    if let Err(e) = write_unit(
        systemd_dir,
        &path_unit,
        &render_path_unit(service, config_dir),
    ) {
        return Err(e);
    }
    if let Err(e) = write_unit(
        systemd_dir,
        &reload_unit_name(service),
        &render_reload_unit(service),
    ) {
        return Err(e);
    }
    Ok(Some(path_unit))
}

/// Delete the Caddy path and reload units that older versions installed.
pub fn remove_caddy_units(systemd_dir: &Path) {
    for name in [
        path_unit_name(CADDY_SERVICE),
        reload_unit_name(CADDY_SERVICE),
    ] {
        let unit = systemd_dir.join(&name);
        if let Err(e) = fs::remove_file(&unit) {
            if e.kind() != std::io::ErrorKind::NotFound {
                tracing::warn!("could not remove {}: {e}", unit.display());
            }
        }
    }
}

/// Path units of the built-in routers other than `router`, to disable when
/// a host switches routers.
pub fn other_path_units(router: &Router) -> Vec<String> {
    [NGINX_SERVICE, CADDY_SERVICE]
        .into_iter()
        .filter(|service| router_service(router) != Some(service))
        .map(path_unit_name)
        .collect()
}

#[cfg(test)]
#[path = "router_units_tests.rs"]
mod tests;
