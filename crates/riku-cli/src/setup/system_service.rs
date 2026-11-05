/// System-wide (root) systemd service installation for riku.
use anyhow::{anyhow, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::router::Router;
use crate::config::{RikuPaths, ACME_WEBROOT, CADDY_SITE_ROOT, SOCKET_ROOT, STATIC_ROOT};
use crate::util::{echo, ROOT_BINARY};

use super::deploy_user::{deploy_user_name, passwd_home, running_as_root};
use super::router_units::{other_path_units, unit_ordering, write_router_units};

pub const SYSTEMD_DIR: &str = "/etc/systemd/system";

fn render_system_unit(
    router: &Router,
    deploy_user: &str,
    deploy_home: &str,
    riku_binary_path: &str,
) -> String {
    format!(
        r#"[Unit]
Description=Riku Process Supervisor
Documentation=https://dreygur.github.io/riku/
{ordering}

[Service]
Type=simple
User={deploy_user}
Group={deploy_user}
WorkingDirectory={deploy_home}
ExecStart={riku_binary_path} supervisor
Restart=always
RestartSec=5
StandardOutput=journal
StandardError=journal
SyslogIdentifier=riku

# Security hardening
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=read-only
PrivateTmp=true

# Allow writing to riku directories. ProtectSystem=strict leaves /run read
# only, and uWSGI copies, which inherit this namespace, bind their sockets in
# the socket directory. Deploys started from here write Caddy site files,
# ACME challenge files and published static files.
# The leading - keeps the unit starting when a directory is missing.
ReadWritePaths={deploy_home}/.riku -{socket_root} -{caddy_site_root} -{acme_webroot} -{static_root}

# Resource limits
MemoryMax=512M
CPUQuota=50%

[Install]
WantedBy=multi-user.target
"#,
        ordering = unit_ordering(router),
        deploy_user = deploy_user,
        deploy_home = deploy_home,
        riku_binary_path = riku_binary_path,
        socket_root = SOCKET_ROOT,
        caddy_site_root = CADDY_SITE_ROOT,
        acme_webroot = ACME_WEBROOT,
        static_root = STATIC_ROOT,
    )
}

/// The reload request file the nginx path unit watches in the deploy
/// user's tree.
fn reload_trigger(deploy_home: &str) -> PathBuf {
    let home = PathBuf::from(deploy_home);
    RikuPaths::from_dirs(home.join(".riku"), &home).nginx_reload_trigger()
}

fn write_system_unit(systemd_dir: &Path, content: &str) -> Result<()> {
    let service_path = systemd_dir.join("riku.service");
    if let Err(e) = fs::write(&service_path, content) {
        return Err(anyhow!("failed to write {}: {e}", service_path.display()));
    }
    echo(
        &format!("✓ Created systemd service at {}", service_path.display()),
        "green",
    );
    Ok(())
}

fn systemctl_ok(args: &[&str]) -> bool {
    Command::new("systemctl")
        .args(args)
        .output()
        .is_ok_and(|output| output.status.success())
}

fn start_riku_service() {
    if systemctl_ok(&["daemon-reload"]) {
        echo("✓ Reloaded systemd daemon", "green");
    }
    if systemctl_ok(&["enable", "riku"]) {
        echo("✓ Enabled riku service (starts on boot)", "green");
    }
    if !systemctl_ok(&["start", "riku"]) {
        return;
    }
    echo("✓ Started riku service", "green");

    std::thread::sleep(std::time::Duration::from_millis(500));
    if let Ok(status) = Command::new("systemctl")
        .args(["is-active", "riku"])
        .output()
    {
        if String::from_utf8_lossy(&status.stdout).trim() == "active" {
            echo("✓ Supervisor daemon is running", "green");
        }
    }
}

/// Enable the watcher for the active router and stop the other built-in
/// router's watcher, which a host switching routers would otherwise keep.
fn enable_router_watch(router: &Router, path_unit: Option<&str>) {
    for stale in other_path_units(router) {
        let _ = systemctl_ok(&["disable", "--now", &stale]);
    }
    let Some(path_unit) = path_unit else {
        return;
    };
    // Restart, not start: a path unit already running keeps watching the
    // path it was started with, and one left failed stays down.
    if systemctl_ok(&["enable", path_unit]) && systemctl_ok(&["restart", path_unit]) {
        echo(
            &format!("✓ Enabled {} auto-reload watcher", router.name()),
            "green",
        );
    }
}

/// Install systemd service files for riku daemon (system-wide, requires root).
#[allow(clippy::question_mark)]
pub fn install_systemd_service(router: &Router) -> Result<()> {
    if !running_as_root() {
        return Ok(());
    }

    let deploy_user = deploy_user_name();
    let deploy_home = passwd_home(&deploy_user).unwrap_or_else(|| format!("/home/{deploy_user}"));
    let systemd_dir = Path::new(SYSTEMD_DIR);
    if let Err(e) = fs::create_dir_all(systemd_dir) {
        return Err(anyhow!("failed to create {SYSTEMD_DIR}: {e}"));
    }

    let unit = render_system_unit(router, &deploy_user, &deploy_home, ROOT_BINARY);
    if let Err(e) = write_system_unit(systemd_dir, &unit) {
        return Err(e);
    }
    let trigger = reload_trigger(&deploy_home);
    let path_unit = match write_router_units(systemd_dir, router, &trigger) {
        Ok(path_unit) => path_unit,
        Err(e) => return Err(e),
    };

    echo("Enabling riku systemd service...", "green");
    start_riku_service();
    enable_router_watch(router, path_unit.as_deref());
    Ok(())
}

#[cfg(test)]
#[path = "system_service_tests.rs"]
mod tests;
