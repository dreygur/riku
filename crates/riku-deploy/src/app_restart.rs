//! Restarting the running workers of a Procfile app, or of one of its
//! process kinds, as `riku restart` does. Nothing is built: the workers start again from the build and the
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
use crate::util::file_names::worker::WorkerName;

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
/// which starts them too. An error when no supervisor runs afterwards.
pub fn restart_configs(configs: &[PathBuf], paths: &RikuPaths) -> Result<()> {
    restart_configs_with(configs, paths, supervisor_ctl::ensure_supervisor_running)
}

/// [`restart_configs`] with the way a missing supervisor is started given,
/// so a test never launches one.
fn restart_configs_with(
    configs: &[PathBuf],
    paths: &RikuPaths,
    start_supervisor: impl Fn(&RikuPaths) -> bool,
) -> Result<()> {
    for config in configs {
        if let Err(e) = bump_mtime(config) {
            return Err(e);
        }
    }
    if !supervisor_ctl::supervisor_is_running(paths) && !start_supervisor(paths) {
        return Err(anyhow!(
            "the supervisor is not running and could not be started, so nothing restarted; \
             see riku doctor"
        ));
    }
    supervisor_ctl::notify_supervisor_reload(paths);
    Ok(())
}

fn config_kind(config: &Path) -> Option<String> {
    config
        .file_stem()
        .and_then(|stem| stem.to_str())
        .and_then(WorkerName::parse)
        .map(|name| name.kind)
}

/// The configs in `enabled` that run `kind`. None of them is an error that
/// names the kinds the app does run.
fn configs_of_kind(app: &str, kind: &str, enabled: Vec<PathBuf>) -> Result<Vec<PathBuf>> {
    let (matching, others): (Vec<PathBuf>, Vec<PathBuf>) = enabled
        .into_iter()
        .partition(|config| config_kind(config).as_deref() == Some(kind));
    if !matching.is_empty() {
        return Ok(matching);
    }
    let mut kinds: Vec<String> = others.iter().filter_map(|c| config_kind(c)).collect();
    kinds.sort_unstable();
    kinds.dedup();
    Err(anyhow!(
        "'{app}' runs no process '{kind}'; its processes are: {}",
        kinds.join(", ")
    ))
}

/// Restart the enabled workers of `app`, or only those of process `kind`
/// (`web`, `worker`, ...), and return how many there are. An app with no
/// enabled workers (never deployed, or stopped) is an error that names
/// `riku deploy`.
pub fn restart_procfile_app(app: &str, kind: Option<&str>, paths: &RikuPaths) -> Result<usize> {
    let enabled = app_worker_configs(&paths.workers_enabled, app);
    if enabled.is_empty() {
        return Err(anyhow!(
            "'{app}' has no running processes to restart; start it with riku deploy {app}"
        ));
    }
    let chosen = match kind {
        Some(kind) => match configs_of_kind(app, kind, enabled) {
            Ok(chosen) => chosen,
            Err(e) => return Err(e),
        },
        None => enabled,
    };
    match restart_configs(&chosen, paths) {
        Ok(()) => Ok(chosen.len()),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
#[path = "app_restart_tests.rs"]
mod tests;
