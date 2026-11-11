//! `riku restart --hot`: restarting an app without dropping requests.

use anyhow::Result;
use riku_supervisor::config::app_worker_configs;
use std::fs;

use super::control_target::{control_target, ControlTarget};
use crate::config::RikuPaths;
use crate::util::{display, exit_if_invalid};

/// Signal the supervisor by updating the mtime of each enabled worker TOML.
/// The supervisor's file watcher (notify) detects the Modify event and
/// reloads the config. A real mtime bump needs the content written back.
fn touch_worker_configs(paths: &RikuPaths, app: &str) {
    let mut count = 0;
    for entry in app_worker_configs(&paths.workers_enabled, app) {
        match fs::read_to_string(&entry) {
            Ok(content) => {
                if let Err(e) = fs::write(&entry, content) {
                    display::warn(&format!("failed to touch {}: {}", entry.display(), e));
                } else {
                    count += 1;
                }
            }
            Err(e) => {
                display::warn(&format!("failed to read {}: {}", entry.display(), e));
            }
        }
    }

    if count > 0 {
        display::success(&format!("Triggered hot reload for {} worker(s)", count));
        display::warn("Note: Supervisor must be running for hot reload to take effect.");
    } else {
        display::warn("No worker configs found. Is the app deployed?");
    }
}

/// Hot reload an app (zero downtime restart). A container app already
/// restarts one copy at a time behind the router, so it gets the same
/// rolling replacement as `riku restart`.
#[allow(clippy::question_mark)]
pub fn cmd_hot_reload(paths: &RikuPaths, app: &str) -> Result<()> {
    let app = match exit_if_invalid(app, &paths.app_root) {
        Ok(app) => app,
        Err(e) => return Err(e),
    };
    display::info(&format!("Hot reloading app '{}'...", app));
    match control_target(paths, &app, None) {
        Ok(ControlTarget::Containers { app, .. }) => {
            crate::deploy::containers::restart_service(&app, paths, None)
        }
        Ok(ControlTarget::Procfile) => {
            touch_worker_configs(paths, &app);
            Ok(())
        }
        Err(e) => Err(e),
    }
}

#[cfg(test)]
#[path = "hot_reload_tests.rs"]
mod tests;
