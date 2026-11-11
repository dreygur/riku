//! Installs the router helper as a systemd service. Root only.
//!
//! The service is `Type=simple`: the helper binds `/run/riku-router.sock`
//! itself and runs until stopped. On a host without systemd init prints the
//! command for the operator to run by hand instead.
//!
//! Security: the unit runs only the root owned `/usr/local/bin/riku` (see
//! [`super::binary`]), so root never executes a file from the deploy user's
//! tree. The deploy user's name goes into the unit text, so it is refused
//! unless it is plain account name characters.

use std::fs;
use std::path::Path;
use std::thread;
use std::time::Duration;

use anyhow::{anyhow, Result};

use crate::config::SYSTEMD_RUNTIME_DIR;
use crate::util::{echo, ROOT_BINARY};

use super::socket_group::is_account_name;
use super::system_service::{systemctl_ok, SYSTEMD_DIR};

const UNIT_NAME: &str = "riku-router-helper.service";
/// Time for a helper that dies on start to be seen as not active.
const SETTLE: Duration = Duration::from_secs(1);

fn render_unit(deploy_user: &str) -> Result<String> {
    if !is_account_name(deploy_user) {
        return Err(anyhow!(
            "refusing to write a unit for deploy user {deploy_user:?}"
        ));
    }
    Ok(format!(
        "[Unit]
Description=Riku router helper
Documentation=https://dreygur.github.io/riku/
After=network.target
Before=riku.service

[Service]
Type=simple
Environment=RIKU_USER={deploy_user}
ExecStart={ROOT_BINARY} router-helper
Restart=always
RestartSec=2
ProtectHome=yes
NoNewPrivileges=yes
PrivateTmp=yes
StandardOutput=journal
StandardError=journal
SyslogIdentifier=riku-router-helper

[Install]
WantedBy=multi-user.target
"
    ))
}

fn write_unit(systemd_dir: &Path, deploy_user: &str) -> Result<()> {
    let text = match render_unit(deploy_user) {
        Ok(text) => text,
        Err(e) => return Err(anyhow!("router helper unit: {e}")),
    };
    let path = systemd_dir.join(UNIT_NAME);
    match fs::write(&path, text) {
        Ok(()) => Ok(()),
        Err(e) => Err(anyhow!("failed to write {}: {e}", path.display())),
    }
}

/// Write the helper unit and start it. Restart, not start, so a service
/// already running picks up a new binary.
fn enable_and_restart(systemd_dir: &Path, deploy_user: &str) -> Result<()> {
    if let Err(e) = write_unit(systemd_dir, deploy_user) {
        return Err(anyhow!("{e}"));
    }
    if !systemctl_ok(&["daemon-reload"]) {
        return Err(anyhow!("systemctl daemon-reload failed"));
    }
    if !systemctl_ok(&["enable", UNIT_NAME]) || !systemctl_ok(&["restart", UNIT_NAME]) {
        return Err(anyhow!("could not enable and start {UNIT_NAME}"));
    }
    thread::sleep(SETTLE);
    if !systemctl_ok(&["is-active", "--quiet", UNIT_NAME]) {
        return Err(anyhow!(
            "{UNIT_NAME} is not running. See: journalctl -u {UNIT_NAME} -n 20"
        ));
    }
    echo("Started the router helper", "green");
    Ok(())
}

/// Install the helper when systemd is running, or say how to run it by hand.
pub fn install_router_helper(deploy_user: &str) -> Result<()> {
    if Path::new(SYSTEMD_RUNTIME_DIR).is_dir() {
        return enable_and_restart(Path::new(SYSTEMD_DIR), deploy_user);
    }
    echo(
        &format!(
            "This host does not run systemd. Start the router helper as root: \
             RIKU_USER={deploy_user} {ROOT_BINARY} router-helper"
        ),
        "yellow",
    );
    Ok(())
}

#[cfg(test)]
#[path = "helper_install_tests.rs"]
mod tests;
