//! Moves a running Caddy to its admin unix socket during init, by restarting it.
//!
//! The admin socket lives in `/run/caddy`. Caddy's packaged `caddy.service`
//! does not declare a runtime directory (I found no copy of it in this
//! repository to check, so this is from the stock Debian and Fedora packages),
//! so init adds a systemd drop-in that does, and creates the directory now
//! for the Caddy that is already running.
//!
//! Security: the drop-in text is a constant written under `/etc/systemd`,
//! and the directory is owned by `caddy` with mode `0750`, so only `caddy`
//! and root reach the admin API. Every command runs with a fixed argv.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::thread;
use std::time::Duration;

use anyhow::{anyhow, Result};
use nix::unistd::User;

use crate::util::{display, socket_dir, write_atomic};

use super::caddy_host::CADDY_USER;
use super::system_service::{systemctl_ok, SYSTEMD_DIR};

const RUN_DIR: &str = "/run/caddy";
const DROP_IN_DIR: &str = "caddy.service.d";
const DROP_IN_FILE: &str = "riku-runtime-dir.conf";
const DROP_IN_TEXT: &str = "[Service]\nRuntimeDirectory=caddy\nRuntimeDirectoryMode=0750\n";
const RUN_DIR_MODE: u32 = 0o750;
const ADMIN_SOCKET: &str = "/run/caddy/admin.sock";
const SOCKET_POLLS: u32 = 20;
const SOCKET_POLL: Duration = Duration::from_millis(250);

fn write_drop_in(systemd_dir: &Path) -> Result<()> {
    let dir = systemd_dir.join(DROP_IN_DIR);
    if let Err(e) = fs::create_dir_all(&dir) {
        return Err(anyhow!("failed to create {}: {e}", dir.display()));
    }
    let file = dir.join(DROP_IN_FILE);
    if fs::read_to_string(&file).is_ok_and(|current| current == DROP_IN_TEXT) {
        return Ok(());
    }
    match write_atomic(&file, DROP_IN_TEXT.as_bytes()) {
        Ok(()) => Ok(()),
        Err(e) => Err(anyhow!("failed to write {}: {e}", file.display())),
    }
}

fn create_run_dir(group: &str) -> Result<()> {
    let caddy = match User::from_name(CADDY_USER) {
        Ok(Some(caddy)) => caddy,
        Ok(None) => return Err(anyhow!("user {CADDY_USER} not found")),
        Err(e) => return Err(anyhow!("failed to look up user {CADDY_USER}: {e}")),
    };
    if let Err(e) = socket_dir::prepare(Path::new(RUN_DIR), Some(caddy.uid.as_raw()), group) {
        return Err(anyhow!("{e}"));
    }
    match fs::set_permissions(RUN_DIR, fs::Permissions::from_mode(RUN_DIR_MODE)) {
        Ok(()) => Ok(()),
        Err(e) => Err(anyhow!("failed to chmod {RUN_DIR}: {e}")),
    }
}

/// Make `/run/caddy` exist for the admin socket, now and after every
/// restart, then restart Caddy so it listens on the socket. A reload cannot do
/// this: it targets the new Caddyfile's socket, while the running Caddy still
/// listens on `localhost:2019`.
pub(super) fn move_admin_to_socket(group: &str) -> Result<()> {
    if let Err(e) = write_drop_in(Path::new(SYSTEMD_DIR)) {
        return Err(anyhow!("{e}"));
    }
    if !systemctl_ok(&["daemon-reload"]) {
        return Err(anyhow!("systemctl daemon-reload failed"));
    }
    if let Err(e) = create_run_dir(group) {
        return Err(anyhow!("{e}"));
    }
    if !systemctl_ok(&["restart", "caddy"]) {
        return Err(anyhow!("could not restart caddy onto the admin socket"));
    }
    if !wait_for_socket(Path::new(ADMIN_SOCKET), SOCKET_POLLS) {
        return Err(anyhow!(
            "caddy did not create {ADMIN_SOCKET}. See: journalctl -u caddy -n 20"
        ));
    }
    display::step_done("Caddy", &format!("restarted, admin on {ADMIN_SOCKET}"));
    Ok(())
}

fn wait_for_socket(socket: &Path, polls: u32) -> bool {
    for _ in 0..polls {
        if socket.exists() {
            return true;
        }
        thread::sleep(SOCKET_POLL);
    }
    socket.exists()
}

#[cfg(test)]
#[path = "caddy_admin_tests.rs"]
mod tests;
