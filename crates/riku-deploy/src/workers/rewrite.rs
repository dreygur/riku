//! Rewriting the configs of a Procfile app's running workers from a new env,
//! for `riku restart`. The copies keep their kind, command and count; only
//! what the env decides changes.
//!
//! Web facing kinds go through the same rolling replace a deploy uses: a new
//! copy starts in a free slot, passes its health check and joins the router
//! before an old one is drained, so the router keeps the kind. A kind whose
//! copies cannot overlap (see [`kinds_down_while_restarting`]) stops the old
//! copy first and is down until its replacement is healthy. Other kinds have
//! each config rewritten in place, which restarts that copy.

// Project rule 16 forbids `?`, and this lint asks for exactly that rewrite.
#![allow(clippy::question_mark)]

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use anyhow::{anyhow, Result};
use riku_container::compose::ReplaceOrder;
use riku_container::types::CopyIndex;
use riku_supervisor::config::WorkerConfig;

use super::files::write_worker_config;
use super::web_env::{build_worker_env, is_web_facing};
use super::web_rollout::{roll_web_kind, web_order, WebDeploy, WebRoutes};
use crate::config::RikuPaths;
use crate::rolling::RouteSink;

/// One running kind: its command and the ordinals of its copies.
struct RunningKind<'a> {
    command: &'a str,
    ordinals: Vec<u32>,
}

fn by_kind(running: &[WorkerConfig]) -> BTreeMap<&str, RunningKind<'_>> {
    let mut kinds: BTreeMap<&str, RunningKind<'_>> = BTreeMap::new();
    for config in running {
        kinds
            .entry(config.worker.kind.as_str())
            .or_insert_with(|| RunningKind {
                command: config.worker.command.as_str(),
                ordinals: Vec::new(),
            })
            .ordinals
            .push(config.worker.ordinal);
    }
    kinds
}

fn rewrite_copy(deploy: &WebDeploy<'_>, kind: &str, command: &str, ordinal: u32) -> Result<()> {
    let copy = match CopyIndex::new(ordinal) {
        Some(copy) => copy,
        None => return Err(anyhow!("{kind} copy {ordinal} is not a valid copy number")),
    };
    let env = match build_worker_env(deploy.app, kind, copy, deploy.env, &[], deploy.paths) {
        Ok(env) => env,
        Err(e) => return Err(e),
    };
    let written = write_worker_config(
        deploy.app,
        deploy.app_path,
        kind,
        command,
        ordinal,
        env,
        deploy.paths,
    );
    match written {
        Ok(_) => Ok(()),
        Err(e) => Err(e),
    }
}

/// The web facing kinds among `kinds` whose old copy is stopped before its
/// replacement starts (a pinned `PORT`, or a shared wsgi socket), so their
/// route is down while they restart.
pub(crate) fn kinds_down_while_restarting<'a>(
    kinds: impl IntoIterator<Item = &'a str>,
    env: &HashMap<String, String>,
) -> Result<Vec<&'a str>> {
    let mut down = Vec::new();
    for kind in kinds.into_iter().filter(|kind| is_web_facing(kind)) {
        match web_order(kind, env) {
            Ok(ReplaceOrder::StopFirst) => down.push(kind),
            Ok(ReplaceOrder::Overlap) => {}
            Err(e) => return Err(e),
        }
    }
    Ok(down)
}

/// Rewrite the configs of the `running` workers of `app` with `env`, the
/// env a deploy would give them, publishing web copies to `routes`. Needs
/// the supervisor running: web copies are only replaced once their
/// successors are healthy.
///
/// The router is checked before any config changes, and web facing kinds
/// roll before the others are rewritten, so a router that is down or would
/// refuse the app leaves every worker as it was.
pub(crate) fn rewrite_running(
    app: &str,
    app_path: &Path,
    running: &[WorkerConfig],
    env: &HashMap<String, String>,
    paths: &RikuPaths,
    routes: &mut impl RouteSink,
) -> Result<()> {
    let deploy = WebDeploy {
        app,
        app_path,
        env,
        paths,
    };
    let (web, other): (Vec<_>, Vec<_>) = by_kind(running)
        .into_iter()
        .partition(|(kind, _)| is_web_facing(kind));
    if !web.is_empty() {
        if let Err(e) = routes.check() {
            return Err(e.context("nothing restarted: the router would refuse the app"));
        }
    }
    for (kind, found) in web {
        let want = found.ordinals.len() as u32;
        if let Err(e) = roll_web_kind(&deploy, kind, found.command, want, routes) {
            return Err(e);
        }
    }
    for (kind, found) in other {
        for ordinal in &found.ordinals {
            if let Err(e) = rewrite_copy(&deploy, kind, found.command, *ordinal) {
                return Err(e);
            }
        }
    }
    Ok(())
}

/// The route sink a Procfile app's web copies publish to: the active router.
pub(crate) fn app_routes<'a>(
    app: &'a str,
    app_path: &'a Path,
    paths: &'a RikuPaths,
) -> impl RouteSink + 'a {
    WebRoutes {
        app,
        app_path,
        paths,
    }
}

#[cfg(test)]
#[path = "rewrite_tests.rs"]
mod tests;
