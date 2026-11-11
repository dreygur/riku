//! User-level (non-root) systemd service setup for riku.

// Project rule 16 forbids `?`, and this lint asks for exactly that rewrite.
#![allow(clippy::question_mark)]

use anyhow::Result;
use std::path::Path;
use std::process::Command;

use crate::config::router::Router;
use crate::config::RikuPaths;
use crate::util::{display, ROOT_BINARY};

use super::outside_supervisor::replace_outside_supervisor;
use super::supervisor_binary::{action_for, service_main_pid, write_unit, UnitScope};

/// Setup systemd service for Riku supervisor (user-level, no root required).
// Project rule 16 forbids `?`, so the early return is spelled out.
#[allow(clippy::question_mark)]
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

    let service_content = render_user_unit(router, &paths.riku_root.to_string_lossy());

    let change = match write_unit(&systemd_dir.join("riku.service"), &service_content) {
        Ok(change) => change,
        Err(e) => return Err(e),
    };
    user_systemctl(&["daemon-reload"]);
    let mut done = vec!["user unit written"];
    if user_systemctl(&["enable", "riku"]) {
        done.push("enabled");
    }
    if let Err(e) = replace_outside_supervisor(paths, UnitScope::User) {
        return Err(e);
    }
    let main_pid = match service_main_pid(UnitScope::User) {
        Ok(pid) => pid,
        Err(e) => return Err(e),
    };
    let action = action_for(main_pid, change, Path::new("/proc"), Path::new(ROOT_BINARY));
    if user_systemctl(&[action.verb(), "riku"]) {
        done.push(action.outcome());
    }
    display::step_done("Service", &done.join(", "));
    Ok(())
}

/// The user unit text. `riku_root` must be absolute: systemd does not
/// expand `~` in `ReadWritePaths` for user services on all distributions.
fn render_user_unit(router: &Router, riku_root: &str) -> String {
    format!(
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

# Hands the unit's cgroup to the supervisor, which gives each isolated
# worker its own cgroup with memory, cpu and pids limits inside it.
Delegate=yes

# Security hardening
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=read-only
PrivateTmp=true

# Allow writing to riku directories
ReadWritePaths={riku_root}

[Install]
WantedBy=default.target
"#,
        riku_path = ROOT_BINARY,
        riku_root = riku_root,
        ordering = super::router_units::unit_ordering(router),
    )
}

/// `systemctl --user <args>` with its chatter kept off the rail.
fn user_systemctl(args: &[&str]) -> bool {
    Command::new("systemctl")
        .arg("--user")
        .args(args)
        .output()
        .is_ok_and(|out| out.status.success())
}

#[cfg(test)]
#[path = "user_service_tests.rs"]
mod tests;
