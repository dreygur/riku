//! Restarting, stopping and scaling the services of a deployed container app
//! without a new build.
//!
//! Each operation holds the app's deploy lock, so it never interleaves with a
//! deploy or another operation on the same app.

use std::fs::File;

use anyhow::Result;
use riku_container::compose::{check_scale, ServiceDef};
use riku_container::engine::ContainerInfo;
use riku_container::types::ServiceName;

use super::app::{is_container_app, load_project, read_scaling, write_scaling};
use super::context::{saved_env, ContainerApp};
use super::engine_open::reclassify;
use super::removal::{app_containers, remove_containers};
use super::rollout::roll_out;
use super::AppName;
use crate::config::RikuPaths;
use crate::rolling::scale_to;
use crate::supervisor_ctl::notify_supervisor_reload;
use crate::util::display;

/// Whether the deployed app `app` runs as riku containers.
pub fn runs_containers(app: &str, paths: &RikuPaths) -> Result<bool> {
    match saved_env(app, paths) {
        Ok(env) => is_container_app(&paths.app_root.join(app), &env),
        Err(e) => Err(e),
    }
}

/// The service names of the deployed container app `app`, from its compose
/// file or its single image.
pub fn service_names(app: &str, paths: &RikuPaths) -> Result<Vec<String>> {
    let env = match saved_env(app, paths) {
        Ok(env) => env,
        Err(e) => return Err(e),
    };
    match load_project(app, &paths.app_root.join(app), &env) {
        Ok(project) => Ok(project
            .services
            .iter()
            .map(|service| service.name.to_string())
            .collect()),
        Err(e) => Err(e),
    }
}

pub(super) fn locked<'a>(
    app: &'a AppName,
    paths: &'a RikuPaths,
) -> Result<(File, ContainerApp<'a>)> {
    let lock = match crate::lock::acquire(app.as_str(), paths) {
        Ok(lock) => lock,
        Err(e) => return Err(e),
    };
    let env = match saved_env(app.as_str(), paths) {
        Ok(env) => env,
        Err(e) => return Err(e),
    };
    match ContainerApp::load(app, paths, env) {
        Ok(ctx) => Ok((lock, ctx)),
        Err(e) => Err(e),
    }
}

fn chosen<'a>(ctx: &'a ContainerApp, service: Option<&ServiceName>) -> Result<Vec<&'a ServiceDef>> {
    match service {
        None => Ok(ctx.project.services.iter().collect()),
        Some(name) => ctx.service(name).map(|def| vec![def]),
    }
}

fn restart_locked(
    ctx: &ContainerApp,
    paths: &RikuPaths,
    service: Option<&ServiceName>,
) -> Result<()> {
    let counts = match read_scaling(ctx.app.as_str(), paths) {
        Ok(counts) => counts,
        Err(e) => return Err(e),
    };
    let services = match chosen(ctx, service) {
        Ok(services) => services,
        Err(e) => return Err(e),
    };
    roll_out(ctx, &counts, &services)
}

/// Replace the copies of `service`, or of every service in start order, with
/// fresh ones from the images they run now. Stopped services start again.
pub fn restart_service(
    app: &AppName,
    paths: &RikuPaths,
    service: Option<&ServiceName>,
) -> Result<()> {
    let (_lock, ctx) = match locked(app, paths) {
        Ok(locked) => locked,
        Err(e) => return Err(e),
    };
    let restarted = restart_locked(&ctx, paths, service);
    notify_supervisor_reload(paths);
    reclassify(&ctx.engine, restarted)
}

/// Position of a container's service in start order; containers of services
/// no longer in the compose file stop first.
fn start_rank(ctx: &ContainerApp, info: &ContainerInfo) -> usize {
    ctx.project
        .services
        .iter()
        .position(|s| Some(&s.name) == info.service.as_ref())
        .map_or(usize::MAX, |n| n + 1)
}

fn stop_locked(ctx: &ContainerApp, paths: &RikuPaths, service: Option<&ServiceName>) -> Result<()> {
    let app = ctx.app;
    if let Some(name) = service {
        if let Err(e) = ctx.service(name) {
            return Err(e);
        }
    }
    let mut targets: Vec<ContainerInfo> = match app_containers(&ctx.engine, app.as_str()) {
        Ok(all) => all
            .into_iter()
            .filter(|info| service.is_none() || info.service.as_ref() == service)
            .collect(),
        Err(e) => return Err(e),
    };
    let routed_hit = match service {
        None => ctx.routed.is_some(),
        Some(name) => ctx.is_routed(name),
    };
    if routed_hit {
        if let Err(e) = crate::router::remove(app.as_str(), paths) {
            return Err(e.context("cannot take the app off the router, nothing was stopped"));
        }
    }
    targets.sort_by_key(|info| std::cmp::Reverse(start_rank(ctx, info)));
    if let Err(e) = remove_containers(&ctx.engine, Some(&ctx.project), &targets) {
        return Err(e);
    }
    display::step_done(
        &format!("Stopped {} containers of '{app}'", targets.len()),
        "",
    );
    Ok(())
}

/// Stop and remove the copies of `service`, or every container of the app.
///
/// When the routed service is among them the app is taken off the router
/// first, so the router never points at a stopped copy. Dependents stop
/// before the services they depend on. Volumes are kept. A daemon that stops
/// answering mid operation is reported as [`super::EngineUnreachable`].
pub fn stop_service(app: &AppName, paths: &RikuPaths, service: Option<&ServiceName>) -> Result<()> {
    let (_lock, ctx) = match locked(app, paths) {
        Ok(locked) => locked,
        Err(e) => return Err(e),
    };
    reclassify(&ctx.engine, stop_locked(&ctx, paths, service))
}

/// Grow or shrink `service` to `copies` without restarting the copies it
/// keeps, and record the count in `SCALING` for later deploys.
///
/// Refused before anything changes when the service cannot run that many
/// copies, such as a storage service above 1.
pub fn scale_service(
    app: &AppName,
    paths: &RikuPaths,
    service: &ServiceName,
    copies: u32,
) -> Result<()> {
    let (_lock, ctx) = match locked(app, paths) {
        Ok(locked) => locked,
        Err(e) => return Err(e),
    };
    let def = match ctx.service(service) {
        Ok(def) => def,
        Err(e) => return Err(e),
    };
    if let Err(e) = check_scale(&ctx.project, service, copies) {
        return Err(e);
    }
    let mut runtime = ctx.runtime(def);
    let mut routes = ctx.routes(service);
    let scaled = scale_to(&mut runtime, &mut routes, copies);
    notify_supervisor_reload(paths);
    if let Err(e) = scaled {
        return Err(e.context(format!("service '{service}'")));
    }
    write_scaling(app.as_str(), paths, service, copies)
}

#[cfg(test)]
#[path = "control_tests.rs"]
mod tests;
