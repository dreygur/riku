//! Restarting the running workers of a Procfile app, or of one of its
//! process kinds, as `riku restart` does.
//!
//! `riku restart` rewrites the running workers' configs from the app's
//! current ENV and riku settings, the env a deploy would compute, so an
//! edited ENV takes effect. Nothing is built, no release or preflight runs,
//! and the checkout is left as it is; the runtime plugin's `env` subcommand
//! is the only plugin call. Web facing kinds are rolled one copy at a time
//! as on a deploy: a new copy joins the router before an old one leaves,
//! except for a kind whose copies cannot overlap (a pinned `PORT`, or a
//! shared wsgi `RIKU_NGINX_SOCKET`), where the old copy stops first and the
//! route is down until its replacement is healthy. Restart warns about such
//! a kind before it starts. A rebuild is `riku deploy`.
//!
//! A single worker restart (the dashboard's button) keeps its config and
//! advances the config file's mtime instead. The supervisor reloads a worker
//! whose config file's mtime advanced, on the file watcher's event or on the
//! next SIGHUP; the enabled entries are symlinks into `workers-available`,
//! so the SIGHUP is what makes it reliable.

// Project rule 16 forbids `?`, and this lint asks for exactly that rewrite.
#![allow(clippy::question_mark)]

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use riku_supervisor::config::{app_worker_configs, WorkerConfig};

use crate::config::RikuPaths;
use crate::util::display;
use crate::util::file_names::worker::WorkerName;
use crate::{env_setup, lock, restart_env, supervisor_ctl, workers};

mod config_file;

use config_file::{bump_mtime, read_config};

/// Fails unless a supervisor runs, starting one with `start_supervisor` when
/// none does.
fn require_supervisor(
    paths: &RikuPaths,
    start_supervisor: impl Fn(&RikuPaths) -> bool,
) -> Result<()> {
    if supervisor_ctl::supervisor_is_running(paths) || start_supervisor(paths) {
        return Ok(());
    }
    Err(anyhow!(
        "the supervisor is not running and could not be started, so nothing restarted; \
         see riku doctor"
    ))
}

/// Restart the workers whose configs are `configs` (enabled entries or the
/// files they point at) as they are. Starts the supervisor when it is not
/// running, which starts them too. An error when no supervisor runs
/// afterwards.
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
    if let Err(e) = require_supervisor(paths, start_supervisor) {
        return Err(e);
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

/// The enabled configs of `app`, or of its process `kind`.
fn chosen_configs(app: &str, kind: Option<&str>, paths: &RikuPaths) -> Result<Vec<PathBuf>> {
    let enabled = app_worker_configs(&paths.workers_enabled, app);
    if enabled.is_empty() {
        return Err(anyhow!(
            "'{app}' has no running processes to restart; start it with riku deploy {app}"
        ));
    }
    match kind {
        Some(kind) => configs_of_kind(app, kind, enabled),
        None => Ok(enabled),
    }
}

/// `▲` for each web facing kind in `running` whose route goes down while it
/// restarts (see [`workers::kinds_down_while_restarting`]).
fn warn_route_down(running: &[WorkerConfig], env: &HashMap<String, String>) -> Result<()> {
    let kinds: BTreeSet<&str> = running.iter().map(|c| c.worker.kind.as_str()).collect();
    let down = match workers::kinds_down_while_restarting(kinds, env) {
        Ok(down) => down,
        Err(e) => return Err(e),
    };
    for kind in down {
        display::warn(&format!(
            "{kind} copies cannot overlap (pinned PORT or shared socket): its route is down while it restarts"
        ));
    }
    Ok(())
}

/// Restart the enabled workers of `app`, or only those of process `kind`
/// (`web`, `worker`, ...), with configs rewritten from the current ENV and
/// riku settings, and return how many there are. An app with no enabled
/// workers (never deployed, or stopped) is an error that names
/// `riku deploy`. Holds the app's deploy lock throughout.
pub fn restart_procfile_app(app: &str, kind: Option<&str>, paths: &RikuPaths) -> Result<usize> {
    restart_procfile_app_with(app, kind, paths, supervisor_ctl::ensure_supervisor_running)
}

/// [`restart_procfile_app`] with the way a missing supervisor is started
/// given, so a test never launches one.
fn restart_procfile_app_with(
    app: &str,
    kind: Option<&str>,
    paths: &RikuPaths,
    start_supervisor: impl Fn(&RikuPaths) -> bool,
) -> Result<usize> {
    let chosen = match chosen_configs(app, kind, paths) {
        Ok(chosen) => chosen,
        Err(e) => return Err(e),
    };
    let _lock = match lock::acquire_for_deploy(app, paths) {
        Ok(lock) => lock,
        Err(e) => return Err(e),
    };
    let mut running: Vec<WorkerConfig> = Vec::with_capacity(chosen.len());
    for config in &chosen {
        match read_config(config, paths) {
            Ok(parsed) => running.push(parsed),
            Err(e) => return Err(e),
        }
    }
    let env = match restart_env::worker_env(app, paths) {
        Ok(env) => env,
        Err(e) => return Err(e),
    };
    if let Err(e) = require_supervisor(paths, start_supervisor) {
        return Err(e);
    }
    if let Err(e) = warn_route_down(&running, &env) {
        return Err(e);
    }
    if let Err(e) = env_setup::write_live_env(app, paths, &env) {
        return Err(e);
    }
    let app_path = paths.app_root.join(app);
    if let Err(e) = workers::rewrite_running(app, &app_path, &running, &env, paths) {
        return Err(e);
    }
    supervisor_ctl::notify_supervisor_reload(paths);
    Ok(running.len())
}

#[cfg(test)]
#[path = "app_restart_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "app_restart_env_tests.rs"]
mod env_tests;
