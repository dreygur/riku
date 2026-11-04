//! systemd units that reload the host router when riku changes its config.
//!
//! nginx gets a path unit watching the reload request file under the deploy
//! user's riku root (`RikuPaths::nginx_reload_trigger`), plus a oneshot
//! service that runs `systemctl reload` on it. Caddy and router plugins get no units: riku reloads them itself
//! and waits for the result.

use std::fs;
use std::path::Path;

use anyhow::{anyhow, Result};

use crate::config::router::Router;

const SYSTEMCTL: &str = "/usr/bin/systemctl";
const NGINX_SERVICE: &str = "nginx";

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

fn render_path_unit(service: &str, trigger: &Path) -> String {
    format!(
        "[Unit]
Description=Watch for Riku {service} reload requests
Documentation=https://dreygur.github.io/riku/
PartOf=riku.service

[Path]
PathModified={trigger}
Unit={reload}

[Install]
WantedBy=multi-user.target
",
        trigger = trigger.display(),
        reload = reload_unit_name(service),
    )
}

/// `StartLimitIntervalSec=0` because several deploys in a few seconds are
/// normal. With systemd's default limit the sixth start in 10s fails the
/// path unit, and every later reload waits for a run that never comes.
fn render_reload_unit(service: &str) -> String {
    format!(
        "[Unit]
Description=Reload {service} when Riku asks for it
Documentation=https://dreygur.github.io/riku/
StartLimitIntervalSec=0

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

/// Write the watcher units for `router`, watching the reload request file
/// `trigger`. Returns the path unit to enable, `None` for Caddy or a plugin
/// router.
#[allow(clippy::question_mark)]
pub fn write_router_units(
    systemd_dir: &Path,
    router: &Router,
    trigger: &Path,
) -> Result<Option<String>> {
    let Some(service) = router_service(router) else {
        return Ok(None);
    };
    let path_unit = path_unit_name(service);
    if let Err(e) = write_unit(systemd_dir, &path_unit, &render_path_unit(service, trigger)) {
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

/// The nginx path unit when `router` is not nginx, to disable when a host
/// switches away from nginx.
pub fn other_path_units(router: &Router) -> Vec<String> {
    match router {
        Router::Nginx => Vec::new(),
        Router::Caddy | Router::Plugin(_) => vec![path_unit_name(NGINX_SERVICE)],
    }
}

#[cfg(test)]
#[path = "router_units_tests.rs"]
mod tests;
