//! Starting a Procfile app's processes on deploy.
//!
//! Web facing copies are replaced one at a time through `ProcessRuntime`;
//! other kinds have their worker configs rewritten, stop first. Scaling
//! delta logic lives in `super::scaling`.

// Project rule 16 forbids `?`, and this lint asks for exactly that rewrite.
#![allow(clippy::question_mark)]

use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;

use riku_container::types::CopyIndex;

use crate::config::RikuPaths;
use crate::rolling::RouteSink;
use crate::util::echo;

pub(crate) use super::scaling::apply_scaling_deltas;

/// The process kind a runtime plugin's start command runs as.
const WEB_KIND: &str = "web";
/// Procfile entries the deploy runs once. As workers, the supervisor would
/// restart them every time they exit.
pub(crate) const ONE_SHOT_KINDS: [&str; 2] = ["release", "preflight"];

mod files;
#[cfg(test)]
mod mock_supervisor;
mod pinned;
mod ports;
pub(crate) mod process_runtime;
mod scaling;
mod socket;
mod stop_wait;
pub(crate) mod web_env;
mod web_rollout;

pub use scaling::read_scaling_count;

use files::{remove_stale_configs, write_worker_config};
use pinned::{check_pinned_port_with, port_in_use};
use socket::check_custom_socket;
use web_env::{build_worker_env, clear_route_env, is_web_facing};
use web_rollout::{roll_web_kind, stop_web_kind, WebDeploy, WebRoutes};

/// Writes `count` configs for a kind no router fronts. Its old copies were
/// removed first (see `remove_stale_configs`), so this is stop first.
fn write_copies(deploy: &WebDeploy<'_>, kind: &str, command: &str, count: u32) -> Result<()> {
    for copy in (1..=count).filter_map(CopyIndex::new) {
        let worker_env =
            match build_worker_env(deploy.app, kind, copy, deploy.env, &[], deploy.paths) {
                Ok(worker_env) => worker_env,
                Err(e) => return Err(e),
            };
        let written = write_worker_config(
            deploy.app,
            deploy.app_path,
            kind,
            command,
            copy.get(),
            worker_env,
            deploy.paths,
        );
        if let Err(e) = written {
            return Err(e);
        }
    }
    Ok(())
}

/// Stops the web facing kinds that still run but left the Procfile, such as
/// `wsgi` after the app moved to `web`.
fn stop_dropped_kinds(deploy: &WebDeploy<'_>, kept: &HashMap<String, String>) -> Result<()> {
    let running = match crate::router::web_copies(deploy.app, deploy.paths) {
        Ok(running) => running,
        Err(e) => return Err(e),
    };
    let mut dropped: Vec<&str> = running
        .iter()
        .map(|config| config.worker.kind.as_str())
        .filter(|kind| !kept.contains_key(*kind))
        .collect();
    dropped.sort_unstable();
    dropped.dedup();
    for kind in dropped {
        if let Err(e) = stop_web_kind(deploy, kind) {
            return Err(e);
        }
    }
    Ok(())
}

/// [`create_workers_generic`] with the route sink web copies publish to.
pub(crate) fn create_workers_with(
    app: &str,
    app_path: &Path,
    processes: Option<&HashMap<String, String>>,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
    start_cmd: Option<&str>,
    routes: &mut impl RouteSink,
) -> Result<()> {
    create_workers_probed(
        app,
        app_path,
        processes,
        env,
        paths,
        start_cmd,
        routes,
        port_in_use,
    )
}

/// [`create_workers_with`] with the probe for a pinned port held by some
/// other process given, so a test of the deploy flow does not depend on
/// which ports the host has free.
#[allow(clippy::too_many_arguments)]
pub(crate) fn create_workers_probed(
    app: &str,
    app_path: &Path,
    processes: Option<&HashMap<String, String>>,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
    start_cmd: Option<&str>,
    routes: &mut impl RouteSink,
    in_use: impl Fn(u16) -> bool,
) -> Result<()> {
    if let Err(e) = check_pinned_port_with(app, env, paths, in_use) {
        return Err(e);
    }
    if let Err(e) = check_custom_socket(app, env, paths) {
        return Err(e);
    }
    remove_stale_configs(app, paths);

    let entries = match (processes, start_cmd) {
        (Some(declared), _) if !declared.is_empty() => declared.clone(),
        (_, Some(cmd)) => HashMap::from([(WEB_KIND.to_string(), cmd.to_string())]),
        (_, None) => {
            echo(
                "-----> No processes declared, skipping process creation",
                "yellow",
            );
            return Ok(());
        }
    };

    if !entries.keys().any(|kind| is_web_facing(kind)) {
        if let Err(e) = clear_route_env(app, paths) {
            return Err(e);
        }
    }

    let deploy = WebDeploy {
        app,
        app_path,
        env,
        paths,
    };
    let processes: Vec<(&String, &String)> = entries
        .iter()
        .filter(|(kind, _)| !ONE_SHOT_KINDS.contains(&kind.as_str()))
        .collect();
    let mut counts = HashMap::new();
    for (kind, _) in &processes {
        match read_scaling_count(paths, app, kind) {
            Ok(count) => counts.insert(kind.to_string(), count),
            Err(e) => return Err(e),
        };
    }

    for (kind, command) in processes {
        let effective_cmd = if command.is_empty() {
            start_cmd.unwrap_or(command.as_str())
        } else {
            command.as_str()
        };
        let count = counts[kind.as_str()];
        let outcome = match (is_web_facing(kind), count) {
            (true, 0) => stop_web_kind(&deploy, kind),
            (true, want) => roll_web_kind(&deploy, kind, effective_cmd, want, routes),
            (false, count) => write_copies(&deploy, kind, effective_cmd, count),
        };
        if let Err(e) = outcome {
            return Err(e);
        }
    }
    stop_dropped_kinds(&deploy, &entries)
}

/// Create worker configs for every entry of `processes`, the app's process
/// list from `riku_config::app_config::load_processes` (riku.toml or Procfile).
///
/// Web facing kinds are replaced one copy at a time: a new copy starts in a
/// free slot, must pass its health check, and joins the router before an old
/// copy is drained and stopped. A pinned `PORT` or shared socket stops the
/// old copy first instead. Other kinds have their configs rewritten, stop
/// first, and kinds no longer declared stop. Copies above the scale count are
/// removed. Needs the supervisor running.
///
/// `start_cmd` is an optional fallback command supplied by the runtime plugin
/// via its `start` subcommand. It fills an entry with an empty command, and
/// runs as the one `web` process when no processes are declared.
pub fn create_workers_generic(
    app: &str,
    app_path: &Path,
    processes: Option<&HashMap<String, String>>,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
    start_cmd: Option<&str>,
) -> Result<()> {
    let mut routes = WebRoutes {
        app,
        app_path,
        paths,
    };
    create_workers_with(app, app_path, processes, env, paths, start_cmd, &mut routes)
}

#[cfg(test)]
#[path = "workers_tests.rs"]
mod tests;
