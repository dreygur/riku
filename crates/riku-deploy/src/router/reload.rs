//! Confirmed router reloads.
//!
//! [`reload`] returns only after the router accepted the reload, so a caller
//! can act on that (drain a copy, report success) without guessing. What
//! "confirmed" means depends on the router:
//!
//! - nginx: the reload signal was delivered and accepted (`systemctl reload
//!   nginx` or `nginx -s reload` exited 0). That does not prove nginx
//!   finished loading the config.
//! - Caddy: `caddy reload` exited 0, which means Caddy loaded the config.
//! - Router plugin: its own `reload` verb exited 0.
//!
//! On a host with systemd, nginx is reloaded by `riku-nginx-reload.service`
//! (see [`nginx_unit`]); otherwise nginx is reloaded directly.
//!
//! Security: every program and argument is fixed. Nothing from an app's ENV
//! reaches a command line. The failure text is written to a file under the
//! riku root, which only the deploy user can write.

use std::fs;
use std::process::Command;

use anyhow::{anyhow, Result};

use super::plugin::reload_plugin;
use crate::caddy::SYSTEM_CADDYFILE;
use crate::config::router::Router;
use crate::config::{ReloadPrograms, RikuPaths};
use crate::util::spawn::output_retrying_etxtbsy;

mod nginx_unit;
mod unit_state;

pub use nginx_unit::Baseline;

/// File under the riku root holding the last failed reload's error text.
pub const ERROR_FILE: &str = "router-reload.err";

fn reload_caddy(programs: &ReloadPrograms) -> Result<()> {
    let output = match output_retrying_etxtbsy(Command::new(&programs.caddy).args([
        "reload",
        "--config",
        SYSTEM_CADDYFILE,
        "--adapter",
        "caddyfile",
    ])) {
        Ok(output) => output,
        Err(e) => return Err(anyhow!("caddy reload failed: could not run caddy: {e}")),
    };
    if output.status.success() {
        return Ok(());
    }
    Err(anyhow!(
        "caddy reload failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    ))
}

fn confirm(router: &Router, paths: &RikuPaths, before: &Baseline) -> Result<()> {
    match (router, before) {
        (Router::Nginx, Baseline::Unit) => nginx_unit::await_unit(paths),
        (Router::Nginx, Baseline::Direct) => {
            crate::nginx::reload_nginx_with(&paths.reload_programs.nginx)
        }
        (Router::Caddy, _) => reload_caddy(&paths.reload_programs),
        (Router::Plugin(name), _) => reload_plugin(name, paths),
    }
}

/// Write `outcome` to the doctor's error file, or clear the file on success.
pub fn record_outcome(paths: &RikuPaths, outcome: &Result<()>) {
    let file = paths.riku_root.join(ERROR_FILE);
    let written = match outcome {
        Ok(()) => match fs::remove_file(&file) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        },
        Err(cause) => {
            fs::create_dir_all(&paths.riku_root).and_then(|_| fs::write(&file, cause.to_string()))
        }
    };
    if let Err(e) = written {
        tracing::warn!("cannot update {}: {e}", file.display());
    }
}

/// Reload `router` and return once it has confirmed the new config.
///
/// `before` is the [`Baseline`] taken before the config was written, and
/// this must be called after the write. The programs run come from
/// `paths.reload_programs`. The last failure is also written to
/// `<riku root>/router-reload.err` for `riku doctor`, and the file is
/// removed on success.
pub fn reload(router: &Router, paths: &RikuPaths, before: &Baseline) -> Result<()> {
    let outcome = confirm(router, paths, before);
    record_outcome(paths, &outcome);
    outcome
}

#[cfg(test)]
#[path = "reload/mock_systemctl.rs"]
pub(super) mod mock_systemctl;

#[cfg(test)]
#[path = "reload_tests.rs"]
mod tests;
