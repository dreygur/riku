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

// Project rule 16 forbids `?`, and this lint asks for exactly that rewrite.
#![allow(clippy::question_mark)]

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

/// Writes the drop-in unless it already has this text. True when it changed.
fn write_drop_in(systemd_dir: &Path) -> Result<bool> {
    let dir = systemd_dir.join(DROP_IN_DIR);
    if let Err(e) = fs::create_dir_all(&dir) {
        return Err(anyhow!("failed to create {}: {e}", dir.display()));
    }
    let file = dir.join(DROP_IN_FILE);
    if fs::read_to_string(&file).is_ok_and(|current| current == DROP_IN_TEXT) {
        return Ok(false);
    }
    match write_atomic(&file, DROP_IN_TEXT.as_bytes()) {
        Ok(()) => Ok(true),
        Err(e) => Err(anyhow!("failed to write {}: {e}", file.display())),
    }
}

/// What init found before deciding how to bring Caddy up to date.
#[derive(Debug, Clone, Copy)]
pub(super) struct CaddyState {
    pub drop_in_changed: bool,
    pub caddyfile_changed: bool,
    /// Caddy runs and its admin socket exists.
    pub on_socket: bool,
}

#[derive(Debug, PartialEq, Eq)]
enum CaddyAction {
    Restart,
    Reload,
    Nothing,
}

/// Only a restart moves Caddy's admin endpoint onto the socket, so a restart
/// happens for that alone; a Caddyfile edit on a Caddy already on the socket
/// is a reload, and an unchanged host is left running.
fn caddy_action(state: CaddyState) -> CaddyAction {
    match state {
        CaddyState {
            drop_in_changed: true,
            ..
        }
        | CaddyState {
            on_socket: false, ..
        } => CaddyAction::Restart,
        CaddyState {
            caddyfile_changed: true,
            ..
        } => CaddyAction::Reload,
        _ => CaddyAction::Nothing,
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
/// restart, and bring Caddy up to date with as little disruption as the
/// change needs (see [`caddy_action`]). A reload cannot move the admin
/// endpoint: it targets the new Caddyfile's socket, while a Caddy not yet
/// moved still listens on `localhost:2019`.
pub(super) fn move_admin_to_socket(group: &str, caddyfile_changed: bool) -> Result<()> {
    let drop_in_changed = match write_drop_in(Path::new(SYSTEMD_DIR)) {
        Ok(changed) => changed,
        Err(e) => return Err(anyhow!("{e}")),
    };
    if let Err(e) = create_run_dir(group) {
        return Err(anyhow!("{e}"));
    }
    let state = CaddyState {
        drop_in_changed,
        caddyfile_changed,
        on_socket: systemctl_ok(&["is-active", "--quiet", "caddy"])
            && Path::new(ADMIN_SOCKET).exists(),
    };
    let done = match caddy_action(state) {
        CaddyAction::Nothing => "running",
        CaddyAction::Reload => match systemctl_ok(&["reload", "caddy"]) {
            true => "reloaded",
            false => {
                return Err(anyhow!(
                    "could not reload caddy. See: journalctl -u caddy -n 20"
                ))
            }
        },
        CaddyAction::Restart => {
            if let Err(e) = restart_with_current_unit() {
                return Err(e);
            }
            "restarted"
        }
    };
    if !wait_for_socket(Path::new(ADMIN_SOCKET), SOCKET_POLLS) {
        return Err(anyhow!(
            "caddy did not create {ADMIN_SOCKET}. See: journalctl -u caddy -n 20"
        ));
    }
    display::step_done("Caddy", &format!("{done}, admin on {ADMIN_SOCKET}"));
    Ok(())
}

/// Restarts Caddy after a `daemon-reload`, every time: an earlier init may
/// have written the drop-in and then failed before reloading, and a rerun
/// sees the drop-in unchanged. The reload does not touch the running Caddy.
fn restart_with_current_unit() -> Result<()> {
    if !systemctl_ok(&["daemon-reload"]) {
        return Err(anyhow!("systemctl daemon-reload failed"));
    }
    if !systemctl_ok(&["restart", "caddy"]) {
        return Err(anyhow!("could not restart caddy onto the admin socket"));
    }
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
