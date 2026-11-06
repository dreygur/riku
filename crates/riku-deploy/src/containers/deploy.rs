//! Deploying a container app: build or pull its images, then replace every
//! service's copies in start order.

use std::collections::{BTreeMap, HashMap};

use anyhow::Result;
use riku_container::compose::{check_scale, ComposeProject, ServiceDef};

use super::app::{read_scaling, wanted};
use super::context::ContainerApp;
use super::procfile_workers::ProcfileWorkers;
use super::release::run_release;
use super::removal::remove_dropped;
use super::retire::retire_procfile_workers;
use super::rollout::roll_out;
use super::{prepare_images, AppName};
use crate::config::RikuPaths;
use crate::supervisor_ctl::notify_supervisor_reload;

/// The runtime name hooks receive for container apps.
pub const CONTAINER_RUNTIME: &str = "container";

fn check_counts(project: &ComposeProject, counts: &BTreeMap<String, u32>) -> Result<()> {
    for service in &project.services {
        if let Err(e) = check_scale(project, &service.name, wanted(counts, &service.name)) {
            return Err(e);
        }
    }
    Ok(())
}

/// Pre build hooks, image pulls and builds, post build hooks, then the
/// Procfile `release` command, in the new image, when there is one.
fn build_images(ctx: &ContainerApp) -> Result<()> {
    let app = ctx.app.as_str();
    let runtime = Some(CONTAINER_RUNTIME);
    if let Err(e) = crate::hooks::run_pre_build(app, &ctx.app_path, ctx.paths, runtime, &ctx.env) {
        return Err(e);
    }
    if let Err(e) = prepare_images(&ctx.engine, ctx.app, &ctx.project, &ctx.container_env) {
        return Err(e);
    }
    if let Err(e) = crate::hooks::run_post_build(app, &ctx.app_path, ctx.paths, runtime, &ctx.env) {
        return Err(e);
    }
    run_release(ctx)
}

/// Copy counts checked, images ready and the release command run.
fn prepare(ctx: &ContainerApp) -> Result<BTreeMap<String, u32>> {
    let counts = match read_scaling(ctx.app.as_str(), ctx.paths) {
        Ok(counts) => counts,
        Err(e) => return Err(e),
    };
    if let Err(e) = check_counts(&ctx.project, &counts) {
        return Err(e);
    }
    match build_images(ctx) {
        Ok(()) => Ok(counts),
        Err(e) => Err(e),
    }
}

/// Replace every service's copies, remove containers of dropped services,
/// and take an app with no routed port off the router.
fn roll_out_app(ctx: &ContainerApp, counts: &BTreeMap<String, u32>) -> Result<()> {
    let services: Vec<&ServiceDef> = ctx.project.services.iter().collect();
    if let Err(e) = roll_out(ctx, counts, &services) {
        return Err(e);
    }
    if let Err(e) = remove_dropped(&ctx.engine, ctx.app.as_str(), &ctx.project) {
        return Err(e);
    }
    match ctx.routed {
        Some(_) => Ok(()),
        None => crate::router::remove(ctx.app.as_str(), ctx.paths),
    }
}

/// Deploy `app` as riku managed containers.
///
/// Every service's copy count
/// comes from `SCALING` (1 when absent) and is checked before anything is
/// built. Services are replaced in start order, one copy at a time; a
/// service with `service_healthy` dependencies starts only once their copies
/// are healthy. A failure stops the deploy and leaves the copies not yet
/// replaced serving. Containers of services the compose file no longer has
/// are removed, and an app with no routed port is taken off the router. An app
/// that ran as a Procfile app has its old workers stopped and deleted once the
/// containers are healthy and routed (see [`super::retire`]). The supervisor
/// is then signalled so it lists the new copies at once.
///
/// Blocks for as long as builds and health waits take, so it must not run on
/// a tokio runtime thread.
pub fn deploy_containers(
    app: &AppName,
    paths: &RikuPaths,
    env: &HashMap<String, String>,
) -> Result<()> {
    let ctx = match ContainerApp::load(app, paths, env.clone()) {
        Ok(ctx) => ctx,
        Err(e) => return Err(e),
    };
    let counts = match prepare(&ctx) {
        Ok(counts) => counts,
        Err(e) => return Err(e),
    };
    let stale = match ProcfileWorkers::load(app.as_str(), paths) {
        Ok(stale) => stale,
        Err(e) => return Err(e),
    };
    let rolled = roll_out_app(&ctx, &counts);
    notify_supervisor_reload(paths);
    match rolled {
        Ok(()) => retire_procfile_workers(&ctx, stale),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
#[path = "deploy_tests.rs"]
mod tests;
