/// System-wide (root) systemd service installation for riku.
use anyhow::Result;
use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::config::RikuPaths;
use crate::util::echo;

/// Install systemd service files for riku daemon (system-wide, requires root).
pub fn install_systemd_service(_paths: &RikuPaths) -> Result<()> {
    // Check if we're running as root (required for systemd installation)
    if env::var("USER").unwrap_or_default() != "root" {
        return Ok(()); // Skip if not root
    }

    let riku_binary = env::current_exe()?;
    let riku_binary_path = riku_binary.to_string_lossy();

    let deploy_user = env::var("RIKU_USER").unwrap_or_else(|_| "deploy".to_string());
    let deploy_home = resolve_deploy_home(&deploy_user);

    let systemd_dir = Path::new("/etc/systemd/system");
    if !systemd_dir.exists() {
        fs::create_dir_all(systemd_dir)?;
    }

    write_system_service_file(systemd_dir, &deploy_user, &deploy_home, &riku_binary_path)?;
    write_nginx_path_unit(systemd_dir, &deploy_home)?;
    write_nginx_reload_service(systemd_dir)?;
    enable_system_services()?;

    Ok(())
}

fn resolve_deploy_home(deploy_user: &str) -> String {
    match Command::new("getent").arg("passwd").arg(deploy_user).output() {
        Ok(output) if output.status.success() => {
            let line = String::from_utf8_lossy(&output.stdout);
            line.split(':')
                .nth(5)
                .unwrap_or(&format!("/home/{}", deploy_user))
                .to_string()
        }
        _ => format!("/home/{}", deploy_user),
    }
}

fn write_system_service_file(
    systemd_dir: &Path,
    deploy_user: &str,
    deploy_home: &str,
    riku_binary_path: &str,
) -> Result<()> {
    let service_content = format!(
        r#"[Unit]
Description=Riku Process Supervisor
Documentation=https://dreygur.github.io/riku/
After=network.target nginx.service
Wants=nginx.service

[Service]
Type=simple
User={deploy_user}
Group={deploy_user}
WorkingDirectory={deploy_home}
ExecStart={riku_bin} supervisor
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
        deploy_user = deploy_user,
        deploy_home = deploy_home,
        riku_bin = riku_binary_path
    );

    let service_path = systemd_dir.join("riku.service");
    fs::write(&service_path, &service_content)?;
    echo(
        &format!("✓ Created systemd service at {}", service_path.display()),
        "green",
    );
    Ok(())
}

fn write_nginx_path_unit(systemd_dir: &Path, deploy_home: &str) -> Result<()> {
    let nginx_path_content = format!(
        r#"[Unit]
Description=Watch for Riku nginx configuration changes
Documentation=https://dreygur.github.io/riku/
PartOf=riku.service

[Path]
PathModified={deploy_home}/.riku/nginx
Unit=riku-nginx-reload.service

[Install]
WantedBy=multi-user.target
"#,
        deploy_home = deploy_home
    );

    let nginx_path = systemd_dir.join("riku-nginx.path");
    fs::write(&nginx_path, nginx_path_content)?;
    Ok(())
}

fn write_nginx_reload_service(systemd_dir: &Path) -> Result<()> {
    let nginx_reload_content = r#"[Unit]
Description=Reload nginx when Riku configuration changes
Documentation=https://dreygur.github.io/riku/

[Service]
Type=oneshot
ExecStart=/usr/bin/systemctl reload nginx

[Install]
WantedBy=multi-user.target
"#;

    let nginx_reload = systemd_dir.join("riku-nginx-reload.service");
    fs::write(&nginx_reload, nginx_reload_content)?;
    Ok(())
}

fn enable_system_services() -> Result<()> {
    echo("Enabling riku systemd service...", "green");

    if let Ok(output) = Command::new("systemctl").arg("daemon-reload").output() {
        if output.status.success() {
            echo("✓ Reloaded systemd daemon", "green");
        }
    }

    if let Ok(output) = Command::new("systemctl").args(["enable", "riku"]).output() {
        if output.status.success() {
            echo("✓ Enabled riku service (starts on boot)", "green");
        }
    }

    if let Ok(output) = Command::new("systemctl").args(["start", "riku"]).output() {
        if output.status.success() {
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
    }

    if let Ok(output) = Command::new("systemctl")
        .args(["enable", "riku-nginx.path"])
        .output()
    {
        if output.status.success() {
            echo("✓ Enabled nginx auto-reload watcher", "green");
        }
    }

    if let Ok(output) = Command::new("systemctl")
        .args(["start", "riku-nginx.path"])
        .output()
    {
        if output.status.success() {
            echo("✓ Started nginx auto-reload watcher", "green");
        }
    }

    Ok(())
}
