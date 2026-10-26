/// User-level (non-root) systemd service setup for riku.
use anyhow::Result;
use std::process::Command;

use crate::config::router::Router;
use crate::config::RikuPaths;
use crate::util::echo;

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

    let riku_binary_path = paths.riku_script.to_string_lossy();
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
        riku_path = riku_binary_path,
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
    echo("      ✓ Service file created", "green");

    let _ = Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .status();

    let status = Command::new("systemctl")
        .args(["--user", "enable", "riku"])
        .status();

    if status.is_ok_and(|s| s.success()) {
        echo("      ✓ Service enabled", "green");
    }

    let status = Command::new("systemctl")
        .args(["--user", "start", "riku"])
        .status();

    if status.is_ok_and(|s| s.success()) {
        echo("      ✓ Service started", "green");
    }

    Ok(())
}
