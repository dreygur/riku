/// System-wide (root) systemd service installation for riku.
use anyhow::{anyhow, Result};
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::config::router::Router;
use crate::config::{ACME_WEBROOT, SOCKET_ROOT, STATIC_ROOT};
use crate::util::{echo, ROOT_BINARY};

use super::deploy_user::{deploy_user_name, passwd_home, running_as_root};
use super::router_units::{remove_stale_units, unit_ordering, STALE_UNITS};

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
# the socket directory. Deploys started from here write
# ACME challenge files and published static files. Caddy site files are
# written by the router helper, not by this service.
# The leading - keeps the unit starting when a directory is missing.
ReadWritePaths={deploy_home}/.riku -{socket_root} -{acme_webroot} -{static_root}

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
        acme_webroot = ACME_WEBROOT,
        static_root = STATIC_ROOT,
    )
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

pub(super) fn systemctl_ok(args: &[&str]) -> bool {
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

/// Stop and delete the reload watcher units an earlier init installed.
/// On a host that never had them the disable calls fail and are ignored.
fn remove_old_watchers(systemd_dir: &Path) -> Result<()> {
    for unit in STALE_UNITS {
        let _ = systemctl_ok(&["disable", "--now", unit]);
    }
    let removed = match remove_stale_units(systemd_dir) {
        Ok(removed) => removed,
        Err(e) => return Err(anyhow!("{e}")),
    };
    if !removed {
        return Ok(());
    }
    let _ = systemctl_ok(&["daemon-reload"]);
    echo("✓ Removed the old nginx reload watcher", "green");
    Ok(())
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

    echo("Enabling riku systemd service...", "green");
    start_riku_service();
    remove_old_watchers(systemd_dir)
}

#[cfg(test)]
#[path = "system_service_tests.rs"]
mod tests;
