//! The restart plan for each running kind of a Procfile app, worked out in
//! memory before any config changes, and applying it one kind at a time.
//!
//! Every non web kind's new config is rendered (env, copy number,
//! serialization) up front, so applying it is only the write. A web facing
//! kind's order is checked up front too, but its configs are written as each
//! copy starts, since a copy's port is picked then.

// Project rule 16 forbids `?`, and this lint asks for exactly that rewrite.
#![allow(clippy::question_mark)]

use std::collections::BTreeMap;

use anyhow::{anyhow, Result};
use riku_container::types::CopyIndex;
use riku_supervisor::config::WorkerConfig;

use super::files::{install_worker_config, render_worker_config, RenderedConfig};
use super::socket::refuse_app_socket;
use super::web_env::{build_worker_env, is_web_facing};
use super::web_rollout::{roll_web_kind, web_order, WebDeploy};
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

/// What restarting one kind takes.
pub(super) enum KindPlan<'a> {
    /// A web facing kind, rolled copy by copy.
    Web {
        kind: &'a str,
        command: &'a str,
        want: u32,
    },
    /// Any other kind, with every copy's new config already rendered.
    Other {
        kind: &'a str,
        configs: Vec<RenderedConfig>,
    },
}

impl KindPlan<'_> {
    pub(super) fn kind(&self) -> &str {
        match self {
            KindPlan::Web { kind, .. } | KindPlan::Other { kind, .. } => kind,
        }
    }

    pub(super) fn is_web(&self) -> bool {
        matches!(self, KindPlan::Web { .. })
    }
}

fn render_copy(
    deploy: &WebDeploy<'_>,
    kind: &str,
    command: &str,
    ordinal: u32,
) -> Result<RenderedConfig> {
    let copy = match CopyIndex::new(ordinal) {
        Some(copy) => copy,
        None => return Err(anyhow!("{kind} copy {ordinal} is not a valid copy number")),
    };
    let env = match build_worker_env(deploy.app, kind, copy, deploy.env, &[], deploy.paths) {
        Ok(env) => env,
        Err(e) => return Err(e),
    };
    render_worker_config(
        deploy.app,
        deploy.app_path,
        kind,
        command,
        ordinal,
        env,
        deploy.paths,
    )
}

fn plan_kind<'a>(
    deploy: &WebDeploy<'_>,
    kind: &'a str,
    found: RunningKind<'a>,
) -> Result<KindPlan<'a>> {
    if is_web_facing(kind) {
        if let Err(e) = refuse_app_socket(deploy.env) {
            return Err(e);
        }
        if let Err(e) = web_order(kind, deploy.env) {
            return Err(e);
        }
        return Ok(KindPlan::Web {
            kind,
            command: found.command,
            want: found.ordinals.len() as u32,
        });
    }
    let mut configs = Vec::with_capacity(found.ordinals.len());
    for ordinal in found.ordinals {
        match render_copy(deploy, kind, found.command, ordinal) {
            Ok(rendered) => configs.push(rendered),
            Err(e) => return Err(e),
        }
    }
    Ok(KindPlan::Other { kind, configs })
}

/// Every running kind's plan, web facing kinds first.
pub(super) fn plan_kinds<'a>(
    deploy: &WebDeploy<'_>,
    running: &'a [WorkerConfig],
) -> Result<Vec<KindPlan<'a>>> {
    let (web, other): (Vec<_>, Vec<_>) = by_kind(running)
        .into_iter()
        .partition(|(kind, _)| is_web_facing(kind));
    let mut plans = Vec::with_capacity(web.len() + other.len());
    for (kind, found) in web.into_iter().chain(other) {
        match plan_kind(deploy, kind, found) {
            Ok(plan) => plans.push(plan),
            Err(e) => return Err(e),
        }
    }
    Ok(plans)
}

pub(super) fn apply_plan(
    deploy: &WebDeploy<'_>,
    plan: &KindPlan<'_>,
    routes: &mut impl RouteSink,
) -> Result<()> {
    match plan {
        KindPlan::Web {
            kind,
            command,
            want,
        } => roll_web_kind(deploy, kind, command, *want, routes),
        KindPlan::Other { configs, .. } => {
            for rendered in configs {
                if let Err(e) = install_worker_config(rendered, deploy.paths) {
                    return Err(e);
                }
            }
            Ok(())
        }
    }
}
