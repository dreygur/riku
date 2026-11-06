//! The reload of a router plugin, and the record of the last router failure.
//!
//! [`reload_router_plugin`] returns only after the plugin's own `reload` verb
//! exited 0, so a caller can act on that (drain a copy, report success)
//! without guessing.
//!
//! nginx and Caddy are not here: the router helper installs and reloads them
//! as root, and the deploy side only maps its answer (see
//! [`super::nginx_site`] and [`super::caddy`]).
//!
//! Security: every program and argument is fixed. Nothing from an app's ENV
//! reaches a command line. The failure text is written to a file under the
//! riku root, which only the deploy user can write.

use std::fs;

use anyhow::Result;

use super::plugin::reload_plugin;
use crate::config::RikuPaths;

/// File under the riku root holding the last failed reload's error text.
pub const ERROR_FILE: &str = "router-reload.err";

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

/// Reload the router plugin `name` through its `reload` verb and return once
/// it has confirmed. The last failure is also written to
/// `<riku root>/router-reload.err` for `riku doctor`, and the file is removed
/// on success.
pub fn reload_router_plugin(name: &str, paths: &RikuPaths) -> Result<()> {
    let outcome = reload_plugin(name, paths);
    record_outcome(paths, &outcome);
    outcome
}

#[cfg(test)]
#[path = "reload_tests.rs"]
mod tests;
