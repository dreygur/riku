//! Replacing the copies of a set of services, in start order.

use std::collections::BTreeMap;

use anyhow::{anyhow, Result};
use riku_container::compose::{replace_order, DependsCondition, ServiceDef};
use riku_container::engine::ContainerId;

use super::app::wanted;
use super::context::ContainerApp;
use crate::rolling::health::{wait_healthy, HealthTarget};
use crate::rolling::{replace_all, CopyHandle, CopyRuntime, RouteSink};
use crate::util::display;
use riku_util::backend::Backend;

fn copies_word(n: u32) -> &'static str {
    if n == 1 {
        "copy"
    } else {
        "copies"
    }
}

fn container_ids(ctx: &ContainerApp, service: &ServiceDef) -> Result<Vec<ContainerId>> {
    match ctx.runtime(service).live() {
        Ok(copies) => Ok(copies
            .into_iter()
            .filter_map(|copy| match copy.handle {
                CopyHandle::Container(id) => Some(id),
                CopyHandle::Process(_) => None,
            })
            .collect()),
        Err(e) => Err(e),
    }
}

/// Blocks until every copy of each `service_healthy` dependency of `service`
/// passes the engine's health check. A dependency copy that exited or turned
/// unhealthy fails at once, so the dependent never starts against it.
pub(super) fn wait_for_dependencies(ctx: &ContainerApp, service: &ServiceDef) -> Result<()> {
    let healthy_deps = service
        .depends_on
        .iter()
        .filter(|dep| dep.condition == DependsCondition::Healthy);
    for dep in healthy_deps {
        let def = match ctx.service(&dep.service) {
            Ok(def) => def,
            Err(e) => return Err(e),
        };
        let ids = match container_ids(ctx, def) {
            Ok(ids) => ids,
            Err(e) => return Err(e),
        };
        if ids.is_empty() {
            return Err(anyhow!(
                "service '{}' waits for '{}' to be healthy, but '{}' has no running copies",
                service.name,
                dep.service,
                dep.service
            ));
        }
        for id in ids {
            if let Err(e) = wait_healthy(&HealthTarget::Engine(id), Some(&ctx.engine)) {
                return Err(anyhow!(
                    "service '{}' waits for '{}' to be healthy: {e}",
                    service.name,
                    dep.service
                ));
            }
        }
    }
    Ok(())
}

fn roll_service(ctx: &ContainerApp, service: &ServiceDef, want: u32) -> Result<()> {
    display::step_done(
        &format!("Starting {} ({want} {})", service.name, copies_word(want)),
        "",
    );
    let mut runtime = ctx.runtime(service);
    let mut routes = ctx.routes(&service.name);
    let order = replace_order(&ctx.project, &service.name);
    match replace_all(&mut runtime, &mut routes, order, want) {
        Ok(()) => Ok(()),
        Err(e) => Err(e.context(format!("service '{}'", service.name))),
    }
}

/// Publish the backends of the running copies of a routed `service` again,
/// so a routing setting change reaches the router when a deploy keeps the
/// copies. Nothing happens for an unrouted service or one with no backend.
pub(super) fn republish_routes(ctx: &ContainerApp, service: &ServiceDef) -> Result<()> {
    if !ctx.is_routed(&service.name) {
        return Ok(());
    }
    let copies = match ctx.runtime(service).live() {
        Ok(copies) => copies,
        Err(e) => return Err(e),
    };
    let backends: Vec<Backend> = copies.into_iter().filter_map(|c| c.backend).collect();
    if backends.is_empty() {
        return Ok(());
    }
    match ctx.routes(&service.name).publish(&backends) {
        Ok(()) => Ok(()),
        Err(e) => Err(e.context(format!("service '{}'", service.name))),
    }
}

/// Replace every copy of `services`, one service after another in the order
/// given (callers pass start order), each at the copy count `counts` names.
/// Stops at the first service that fails; the services after it keep their
/// old copies.
pub(crate) fn roll_out(
    ctx: &ContainerApp,
    counts: &BTreeMap<String, u32>,
    services: &[&ServiceDef],
) -> Result<()> {
    for service in services {
        if let Err(e) = wait_for_dependencies(ctx, service) {
            return Err(e);
        }
        if let Err(e) = roll_service(ctx, service, wanted(counts, &service.name)) {
            return Err(e);
        }
    }
    Ok(())
}
