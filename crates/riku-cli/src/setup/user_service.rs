/// User-level (non-root) systemd service setup for riku.
use anyhow::Result;
use std::path::Path;
use std::process::Command;

use crate::config::router::Router;
use crate::config::RikuPaths;
use crate::util::{display, ROOT_BINARY};

use super::outside_supervisor::replace_outside_supervisor;
use super::supervisor_binary::{action_for, service_main_pid, UnitScope};

/// Setup systemd service for Riku supervisor (user-level, no root required).
pub fn setup_systemd_service(paths: &RikuPaths, router: &Router) -> Result<()> {
    let Some(home_dir) = dirs::home_dir() else {
        return Err(anyhow::anyhow!("Could not determine home directory"));
    };
    let systemd_dir = home_dir.join(".config/systemd/user");

    if let Err(e) = std::fs::create_dir_all(&systemd_dir) {
        return Err(anyhow::anyhow!(
            "failed to create {}: {e}",
            systemd_dir.display()
        ));
    }

    let riku_root_abs = paths.riku_root.to_string_lossy();

    let service_content = format!(
        r#"[Unit]
Description=Riku Process Supervisor
Documentation=https://dreygur.github.io/riku/
{ordering}

[Service]
Type=simple
ExecStart={riku_path} supervisor
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

# Allow writing to riku directories (absolute path required, ~ is not expanded
# by systemd in ReadWritePaths for user services on all distributions)
ReadWritePaths={riku_root}

# Resource limits
MemoryMax=512M
CPUQuota=50%

[Install]
WantedBy=default.target
"#,
        riku_path = ROOT_BINARY,
        riku_root = riku_root_abs,
        ordering = super::router_units::unit_ordering(router),
    );

    let service_file = systemd_dir.join("riku.service");
    if let Err(e) = std::fs::write(&service_file, &service_content) {
        return Err(anyhow::anyhow!(
            "failed to write {}: {e}",
            service_file.display()
        ));
    }
    user_systemctl(&["daemon-reload"]);
    let mut done = vec!["user unit written"];
    if user_systemctl(&["enable", "riku"]) {
        done.push("enabled");
    }
    replace_outside_supervisor(paths, UnitScope::User);
    let main_pid = service_main_pid(UnitScope::User);
    let action = action_for(main_pid, Path::new("/proc"), Path::new(ROOT_BINARY));
    if user_systemctl(&[action.verb(), "riku"]) {
        done.push(action.outcome());
    }
    display::step_done("Service", &done.join(", "));
    Ok(())
}

/// `systemctl --user <args>` with its chatter kept off the rail.
fn user_systemctl(args: &[&str]) -> bool {
    Command::new("systemctl")
        .arg("--user")
        .args(args)
        .output()
        .is_ok_and(|out| out.status.success())
}
