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

use crate::config::RikuPaths;
use crate::rolling::RouteSink;
use crate::util::display;

pub(crate) use super::scaling::record_scaling_deltas;

/// The process kind a runtime plugin's start command runs as.
const WEB_KIND: &str = "web";
/// Procfile entries the deploy runs once. As workers, the supervisor would
/// restart them every time they exit.
pub(crate) const ONE_SHOT_KINDS: [&str; 2] = ["release", "preflight"];

/// Whether `kind` runs as long lived copies that scaling counts: not a one
/// shot command and not a cron entry.
pub fn is_scalable_kind(kind: &str) -> bool {
    !ONE_SHOT_KINDS.contains(&kind) && !kind.starts_with("cron")
}

mod copies;
mod files;
#[cfg(test)]
pub(crate) mod mock_supervisor;
mod pinned;
mod plan;
mod ports;
pub(crate) mod process_runtime;
mod rewrite;
mod rewrite_plan;
mod scaling;
mod socket;
mod stop_wait;
pub(crate) mod web_env;
mod web_rollout;

use pinned::port_in_use;
use plan::plan_workers;
pub(crate) use rewrite::{
    app_routes, kinds_down_while_restarting, rewrite_running, PartialRewrite, RewriteError,
};
pub use scaling::read_scaling_count;

use copies::{stop_dropped_kinds, write_copies};
use files::remove_stale_configs;
use web_env::{clear_route_env, is_web_facing};
use web_rollout::{roll_web_kind, stop_web_kind, WebDeploy};

/// Run every check [`create_workers_generic`] makes before it changes a
/// config, without changing anything, so a deploy can tell a refusal from a
/// failure that left workers half replaced.
pub(crate) fn check_workers(
    app: &str,
    processes: Option<&HashMap<String, String>>,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
    start_cmd: Option<&str>,
) -> Result<()> {
    match plan_workers(app, processes, env, paths, start_cmd, port_in_use) {
        Ok(_) => Ok(()),
        Err(e) => Err(e),
    }
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
    let plan = match plan_workers(app, processes, env, paths, start_cmd, in_use) {
        Ok(plan) => plan,
        Err(e) => return Err(e),
    };
    remove_stale_configs(app, paths);
    let plan = match plan {
        Some(plan) => plan,
        None => {
            display::warn("No processes declared, skipping process creation");
            return Ok(());
        }
    };

    if !plan.entries.keys().any(|kind| is_web_facing(kind)) {
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
    for planned in &plan.kinds {
        let (kind, command) = (planned.kind.as_str(), planned.command.as_str());
        let outcome = match (is_web_facing(kind), planned.count) {
            (true, 0) => stop_web_kind(&deploy, kind),
            (true, want) => roll_web_kind(&deploy, kind, command, want, routes),
            (false, count) => write_copies(&deploy, kind, command, count),
        };
        if let Err(e) = outcome {
            return Err(e);
        }
    }
    stop_dropped_kinds(&deploy, &plan.entries)
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
    let mut routes = app_routes(app, app_path, paths);
    create_workers_with(app, app_path, processes, env, paths, start_cmd, &mut routes)
}

#[cfg(test)]
#[path = "workers_tests.rs"]
mod tests;
