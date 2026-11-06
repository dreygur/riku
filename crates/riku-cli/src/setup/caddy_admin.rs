//! Moves a running Caddy to its admin unix socket during init.
//!
//! The admin socket lives in `/run/caddy`. Caddy's packaged `caddy.service`
//! does not declare a runtime directory (I found no copy of it in this
//! repository to check, so this is from the stock Debian and Fedora packages),
//! so init adds a systemd drop-in that does, and creates the directory now
//! for the Caddy that is already running.
//!
//! Security: the drop-in text is a constant written under `/etc/systemd`,
//! and the directory is owned by `caddy` with mode `2750`, so only `caddy`
//! and root reach the admin API. Every command runs with a fixed argv.

use std::fs;
use std::path::Path;

use anyhow::{anyhow, Result};
use nix::unistd::User;

use crate::caddy::SYSTEM_CADDYFILE;
use crate::util::{echo, socket_dir, write_atomic};

use super::caddy_host::{run, CADDY_USER};
use super::system_service::{systemctl_ok, SYSTEMD_DIR};

const RUN_DIR: &str = "/run/caddy";
const DROP_IN_DIR: &str = "caddy.service.d";
const DROP_IN_FILE: &str = "riku-runtime-dir.conf";
const DROP_IN_TEXT: &str = "[Service]\nRuntimeDirectory=caddy\n";

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
    socket_dir::prepare(Path::new(RUN_DIR), Some(caddy.uid.as_raw()), group)
}

/// Make `/run/caddy` exist for the admin socket, now and after every
/// restart, then reload Caddy at its current admin address so it moves to the
/// socket without waiting for the next restart.
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
    let reloaded = run(
        "caddy",
        &[
            "reload",
            "--config",
            SYSTEM_CADDYFILE,
            "--adapter",
            "caddyfile",
        ],
    );
    if let Err(e) = reloaded {
        return Err(anyhow!("could not move Caddy to the admin socket: {e}"));
    }
    echo("      ✓ Caddy reloaded on the admin socket", "green");
    Ok(())
}

#[cfg(test)]
#[path = "caddy_admin_tests.rs"]
mod tests;
