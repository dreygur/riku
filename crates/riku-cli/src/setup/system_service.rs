/// System-wide (root) systemd service installation for riku.
use anyhow::{anyhow, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::router::Router;
use crate::config::RikuPaths;
use crate::util::echo;

use super::binary::INSTALL_DIR;
use super::deploy_user::{deploy_user_name, passwd_home, running_as_root};
use super::router_units::{other_path_units, unit_ordering, write_router_units};

const SYSTEMD_DIR: &str = "/etc/systemd/system";

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

# Allow writing to riku directories
ReadWritePaths={deploy_home}/.riku

# Resource limits
MemoryMax=512M
CPUQuota=50%

[Install]
WantedBy=multi-user.target
"#,
        ordering = unit_ordering(router),
    )
}

/// The directory the router's path unit watches in the deploy user's tree.
fn router_config_dir(router: &Router, deploy_home: &str) -> PathBuf {
    let home = PathBuf::from(deploy_home);
    let paths = RikuPaths::from_dirs(home.join(".riku"), &home);
    match router {
        Router::Caddy => paths.caddy_root,
        Router::Nginx | Router::Plugin(_) => paths.nginx_root,
    }
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
    if systemctl_ok(&["enable", "--now", path_unit]) {
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
    // The binary init installed for the deploy user, not root's own copy:
    // the unit runs as that user, who may not be able to reach root's.
    let riku_binary = Path::new(&deploy_home).join(INSTALL_DIR).join("riku");

    let systemd_dir = Path::new(SYSTEMD_DIR);
    if let Err(e) = fs::create_dir_all(systemd_dir) {
        return Err(anyhow!("failed to create {SYSTEMD_DIR}: {e}"));
    }

    let unit = render_system_unit(
        router,
        &deploy_user,
        &deploy_home,
        &riku_binary.to_string_lossy(),
    );
    if let Err(e) = write_system_unit(systemd_dir, &unit) {
        return Err(e);
    }
    let config_dir = router_config_dir(router, &deploy_home);
    let path_unit = match write_router_units(systemd_dir, router, &config_dir) {
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
