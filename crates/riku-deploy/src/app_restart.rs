//! Restarting the running workers of a Procfile app, as `riku restart`
//! does. Nothing is built: the workers start again from the build and the
//! configs the last deploy left. A rebuild is `riku deploy`.
//!
//! The supervisor reloads a worker whose config file's mtime advanced, on
//! the file watcher's event or on the next SIGHUP. The enabled entries are
//! symlinks into `workers-available`, so bumping the target's mtime raises
//! no event in the watched directory; the SIGHUP is what makes it reliable.

// Project rule 16 forbids `?`, and this lint asks for exactly that rewrite.
#![allow(clippy::question_mark)]

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{anyhow, Result};
use riku_supervisor::config::app_worker_configs;

use crate::config::RikuPaths;
use crate::supervisor_ctl;

fn bump_mtime(config: &Path) -> Result<()> {
    let file = match fs::File::options().write(true).open(config) {
        Ok(file) => file,
        Err(e) => return Err(anyhow!("cannot open {}: {e}", config.display())),
    };
    match file.set_modified(SystemTime::now()) {
        Ok(()) => Ok(()),
        Err(e) => Err(anyhow!("cannot touch {}: {e}", config.display())),
    }
}

/// Restart the workers whose configs are `configs` (enabled entries or the
/// files they point at). Starts the supervisor when it is not running,
/// which starts them too.
pub fn restart_configs(configs: &[PathBuf], paths: &RikuPaths) -> Result<()> {
    for config in configs {
        if let Err(e) = bump_mtime(config) {
            return Err(e);
        }
    }
    supervisor_ctl::start_supervisor(paths);
    supervisor_ctl::notify_supervisor_reload(paths);
    Ok(())
}

/// Restart every enabled worker of `app` and return how many there are. An
/// app with no enabled workers (never deployed, or stopped) is an error
/// that names `riku deploy`.
pub fn restart_procfile_app(app: &str, paths: &RikuPaths) -> Result<usize> {
    let enabled = app_worker_configs(&paths.workers_enabled, app);
    if enabled.is_empty() {
        return Err(anyhow!(
            "'{app}' has no running processes to restart; start it with riku deploy {app}"
        ));
    }
    match restart_configs(&enabled, paths) {
        Ok(()) => Ok(enabled.len()),
        Err(e) => Err(e),
    }
}
