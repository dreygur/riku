//! Starting what the release command talks to before it runs.
//!
//! A release that migrates a database needs that database up. On a first
//! deploy nothing runs yet, so the services the release's service depends
//! on, followed through `depends_on`, are started first through the normal
//! rollout. A dependency that already has a running copy is left alone.

use std::collections::{BTreeMap, HashSet};

use anyhow::Result;
use riku_container::compose::{ComposeProject, ServiceDef};
use riku_container::engine::ContainerState;
use riku_container::types::ServiceName;

use super::context::ContainerApp;
use super::rollout::{roll_out, wait_for_dependencies};

/// Every service `service` depends on, directly or through another one, in
/// start order.
fn dependencies<'a>(project: &'a ComposeProject, service: &ServiceDef) -> Vec<&'a ServiceDef> {
    let mut needed: HashSet<&ServiceName> =
        service.depends_on.iter().map(|dep| &dep.service).collect();
    for candidate in project.services.iter().rev() {
        if needed.contains(&candidate.name) {
            needed.extend(candidate.depends_on.iter().map(|dep| &dep.service));
        }
    }
    project
        .services
        .iter()
        .filter(|s| needed.contains(&s.name))
        .collect()
}

fn is_up(state: &ContainerState) -> bool {
    matches!(
        state,
        ContainerState::Running
            | ContainerState::Starting
            | ContainerState::Unhealthy
            | ContainerState::Restarting
    )
}

/// Names of the services with at least one copy up, healthy or not.
fn running_services(ctx: &ContainerApp) -> Result<HashSet<ServiceName>> {
    let ids = match ctx.engine.list(ctx.app.as_str()) {
        Ok(ids) => ids,
        Err(e) => return Err(e),
    };
    let infos = match ctx.engine.inspect(&ids) {
        Ok(infos) => infos,
        Err(e) => return Err(e),
    };
    Ok(infos
        .into_iter()
        .filter(|info| is_up(&info.state))
        .filter_map(|info| info.service)
        .collect())
}

/// Start the dependencies of `service` that have no copy up, at the
/// copy counts `counts` names, then wait for its `service_healthy`
/// dependencies. Returns the services it started, which the rollout after
/// the release then skips.
pub(super) fn start_release_dependencies(
    ctx: &ContainerApp,
    service: &ServiceDef,
    counts: &BTreeMap<String, u32>,
) -> Result<Vec<ServiceName>> {
    let running = match running_services(ctx) {
        Ok(running) => running,
        Err(e) => return Err(e),
    };
    let stopped: Vec<&ServiceDef> = dependencies(&ctx.project, service)
        .into_iter()
        .filter(|dep| !running.contains(&dep.name))
        .collect();
    if let Err(e) = roll_out(ctx, counts, &stopped) {
        return Err(e);
    }
    if let Err(e) = wait_for_dependencies(ctx, service) {
        return Err(e);
    }
    Ok(stopped.iter().map(|dep| dep.name.clone()).collect())
}
