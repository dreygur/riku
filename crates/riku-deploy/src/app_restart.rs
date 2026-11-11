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
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{anyhow, Result};
use riku_supervisor::config::app_worker_configs;

use crate::config::RikuPaths;
use crate::supervisor_ctl;
use crate::util::file_names::worker::WorkerName;

/// The config file `config` leads to: the target of an enabled link, or the
/// file itself. It must sit directly in `workers_available`; the returned
/// path is under the canonical form of that directory, so only its last
/// component can still be a link, and opening refuses that.
fn config_in_available(config: &Path, paths: &RikuPaths) -> Result<PathBuf> {
    let target = match fs::read_link(config) {
        Ok(target) => match config.parent() {
            Some(dir) => dir.join(target),
            None => target,
        },
        Err(_) => config.to_path_buf(),
    };
    let (dir, name) = match (target.parent(), target.file_name()) {
        (Some(dir), Some(name)) => (dir, name),
        _ => return Err(anyhow!("{} is not a worker config", config.display())),
    };
    let available = fs::canonicalize(&paths.workers_available);
    match (fs::canonicalize(dir), available) {
        (Ok(dir), Ok(available)) if dir == available => Ok(available.join(name)),
        _ => Err(anyhow!(
            "{} does not lead to a config in {}",
            config.display(),
            paths.workers_available.display()
        )),
    }
}

/// Advance the mtime of the config `config` leads to, so the supervisor
/// reloads that worker. Opened read only, non blocking and without following
/// a final link; refused unless it is a regular file this user owns.
fn bump_mtime(config: &Path, paths: &RikuPaths) -> Result<()> {
    let target = match config_in_available(config, paths) {
        Ok(target) => target,
        Err(e) => return Err(e),
    };
    let opened = fs::File::options()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
        .open(&target);
    let file = match opened {
        Ok(file) => file,
        Err(e) => return Err(anyhow!("cannot open {}: {e}", target.display())),
    };
    let meta = match file.metadata() {
        Ok(meta) => meta,
        Err(e) => return Err(anyhow!("cannot stat {}: {e}", target.display())),
    };
    if !meta.is_file() {
        return Err(anyhow!("{} is not a regular file", target.display()));
    }
    if meta.uid() != nix::unistd::geteuid().as_raw() {
        return Err(anyhow!("{} is not owned by this user", target.display()));
    }
    match file.set_modified(SystemTime::now()) {
        Ok(()) => Ok(()),
        Err(e) => Err(anyhow!("cannot touch {}: {e}", target.display())),
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
        if let Err(e) = bump_mtime(config, paths) {
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
