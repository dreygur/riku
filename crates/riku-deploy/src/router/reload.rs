//! Confirmed reloads of the routers the deploy user reloads itself.
//!
//! Each function returns only after the router accepted the reload, so a
//! caller can act on that (drain a copy, report success) without guessing:
//!
//! - Caddy: `caddy reload` exited 0, which means Caddy loaded the config.
//! - Router plugin: its own `reload` verb exited 0.
//!
//! nginx is not here: the router helper installs, checks and reloads it as
//! root, and the deploy side only maps its answer (see [`super::nginx_site`]).
//!
//! Security: every program and argument is fixed. Nothing from an app's ENV
//! reaches a command line. The failure text is written to a file under the
//! riku root, which only the deploy user can write.

use std::fs;
use std::process::Command;

use anyhow::{anyhow, Result};

use super::plugin::reload_plugin;
use crate::caddy::SYSTEM_CADDYFILE;
use crate::config::{ReloadPrograms, RikuPaths};
use crate::util::spawn::output_retrying_etxtbsy;

/// File under the riku root holding the last failed reload's error text.
pub const ERROR_FILE: &str = "router-reload.err";

fn run_caddy_reload(programs: &ReloadPrograms) -> Result<()> {
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

/// Reload Caddy and return once it has confirmed the new config. The last
/// failure is also written to `<riku root>/router-reload.err` for
/// `riku doctor`, and the file is removed on success.
pub fn reload_caddy(paths: &RikuPaths) -> Result<()> {
    let outcome = run_caddy_reload(&paths.reload_programs);
    record_outcome(paths, &outcome);
    outcome
}

/// [`reload_caddy`] for the router plugin `name` and its `reload` verb.
pub fn reload_router_plugin(name: &str, paths: &RikuPaths) -> Result<()> {
    let outcome = reload_plugin(name, paths);
    record_outcome(paths, &outcome);
    outcome
}

#[cfg(test)]
#[path = "reload_tests.rs"]
mod tests;
