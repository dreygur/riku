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

use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use riku_container::compose::ReplaceOrder;
use riku_supervisor::config::WorkerConfig;

use super::rewrite_plan::{apply_plan, plan_kinds, KindPlan};
use super::web_env::is_web_facing;
use super::web_rollout::{web_order, WebDeploy, WebRoutes};
use crate::config::RikuPaths;
use crate::rolling::RouteSink;

/// How a rewrite failed, and whether it changed anything first.
#[derive(Debug)]
pub(crate) enum RewriteError {
    /// Nothing changed: a config could not be rendered, or the router would
    /// refuse the app.
    Untouched(anyhow::Error),
    /// A step failed after configs started changing.
    Partial(PartialRewrite),
}

/// A rewrite that stopped part way.
#[derive(Debug)]
pub(crate) struct PartialRewrite {
    pub cause: anyhow::Error,
    /// The kinds whose every copy now runs the new env.
    pub restarted: Vec<String>,
    /// The kind that failed and the kinds never reached. Their copies may
    /// still run the old env.
    pub left_old: Vec<String>,
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

fn kind_names(plans: &[KindPlan<'_>]) -> Vec<String> {
    plans.iter().map(|plan| plan.kind().to_string()).collect()
}

/// Rewrite the configs of the `running` workers of `app` with `env`, the
/// env a deploy would give them, publishing web copies to `routes`. Needs
/// the supervisor running: web copies are only replaced once their
/// successors are healthy.
///
/// Every other kind's config is rendered in memory and the router is checked
/// before any config changes, so a bad env or a router that is down or would
/// refuse the app leaves every worker as it was ([`RewriteError::Untouched`]).
/// Web facing kinds then roll before the others are written; a failure from
/// there on is [`RewriteError::Partial`], naming which kinds made it.
pub(crate) fn rewrite_running(
    app: &str,
    app_path: &Path,
    running: &[WorkerConfig],
    env: &HashMap<String, String>,
    paths: &RikuPaths,
    routes: &mut impl RouteSink,
) -> std::result::Result<(), RewriteError> {
    let deploy = WebDeploy {
        app,
        app_path,
        env,
        paths,
    };
    let plans = match plan_kinds(&deploy, running) {
        Ok(plans) => plans,
        Err(e) => return Err(RewriteError::Untouched(e.context("nothing restarted"))),
    };
    if plans.iter().any(KindPlan::is_web) {
        if let Err(e) = routes.check() {
            return Err(RewriteError::Untouched(
                e.context("nothing restarted: the router would refuse the app"),
            ));
        }
    }
    for (done, plan) in plans.iter().enumerate() {
        if let Err(cause) = apply_plan(&deploy, plan, routes) {
            return Err(RewriteError::Partial(PartialRewrite {
                cause,
                restarted: kind_names(&plans[..done]),
                left_old: kind_names(&plans[done..]),
            }));
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
