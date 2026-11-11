/// System-wide (root) systemd service installation for riku.
use anyhow::{anyhow, Result};
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::config::router::Router;
use crate::config::{ACME_WEBROOT, SOCKET_ROOT, STATIC_ROOT};
use crate::util::{display, ROOT_BINARY};

use super::deploy_user::{deploy_user_name, passwd_home, running_as_root};
use super::router_units::{remove_stale_units, unit_ordering, STALE_UNITS};
use super::supervisor_binary::{action_for, service_main_pid, UnitScope};

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

# Hands the unit's cgroup to the supervisor, which gives each isolated
# worker its own cgroup with memory, cpu and pids limits inside it.
Delegate=yes

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
    Ok(())
}

pub(super) fn systemctl_ok(args: &[&str]) -> bool {
    Command::new("systemctl")
        .args(args)
        .output()
        .is_ok_and(|output| output.status.success())
}

/// Reload, enable and start `riku.service`, then print one step saying
/// which of those worked. A supervisor that already runs is restarted only
/// when it runs another binary than [`ROOT_BINARY`] (see
/// [`super::supervisor_binary`]). Whether the supervisor stays up is checked
/// by the init summary.
fn start_riku_service() {
    let _ = systemctl_ok(&["daemon-reload"]);
    let mut done = vec!["riku.service written"];
    if systemctl_ok(&["enable", "riku"]) {
        done.push("enabled");
    }
    let main_pid = service_main_pid(UnitScope::System);
    let action = action_for(main_pid, Path::new("/proc"), Path::new(ROOT_BINARY));
    if systemctl_ok(&[action.verb(), "riku"]) {
        done.push(action.outcome());
    }
    display::step_done("Service", &done.join(", "));
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
    display::detail("Removed the old nginx reload watcher");
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

    start_riku_service();
    remove_old_watchers(systemd_dir)
}

#[cfg(test)]
#[path = "system_service_tests.rs"]
mod tests;
