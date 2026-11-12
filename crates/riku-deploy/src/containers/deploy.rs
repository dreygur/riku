//! Deploying a container app: build or pull its images, then replace the
//! copies of every service that changed, in start order.

use std::collections::{BTreeMap, HashMap};

use anyhow::Result;
use riku_container::compose::{check_scale, ComposeProject, ServiceDef};
use riku_container::types::ServiceName;

use super::app::{read_scaling, wanted};
use super::changes::changed_services;
use super::context::ContainerApp;
use super::procfile_workers::ProcfileWorkers;
use super::release::run_release;
use super::removal::remove_dropped;
use super::retire::retire_procfile_workers;
use super::rollout::{republish_routes, roll_out};
use super::{prepare_images, AppName};
use crate::config::RikuPaths;
use crate::supervisor_ctl::notify_supervisor_reload;
use crate::util::display::{self, OutputTail};

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

/// Pre build hooks, image pulls and builds, then post build hooks.
fn build_images(ctx: &ContainerApp) -> Result<()> {
    let app = ctx.app.as_str();
    let runtime = Some(CONTAINER_RUNTIME);
    if let Err(e) = crate::hooks::run_pre_build(app, &ctx.app_path, ctx.paths, runtime, &ctx.env) {
        return Err(e);
    }
    if let Err(e) = prepare_images(&ctx.engine, ctx.app, &ctx.project, &ctx.container_env) {
        return Err(e);
    }
    crate::hooks::run_post_build(app, &ctx.app_path, ctx.paths, runtime, &ctx.env)
}

/// What the rollout works from once images and the release are done.
struct Prepared {
    counts: BTreeMap<String, u32>,
    /// Services started for the release command, already at the new spec.
    started: Vec<ServiceName>,
}

/// Copy counts checked, images ready and the Procfile `release` command run
/// in the new image, when there is one.
fn prepare(ctx: &ContainerApp, recent: &OutputTail) -> Result<Prepared> {
    let counts = match read_scaling(ctx.app.as_str(), ctx.paths) {
        Ok(counts) => counts,
        Err(e) => return Err(e),
    };
    if let Err(e) = check_counts(&ctx.project, &counts) {
        return Err(e);
    }
    if let Err(e) = build_images(ctx) {
        return Err(e);
    }
    match run_release(ctx, &counts, recent) {
        Ok(started) => Ok(Prepared { counts, started }),
        Err(e) => Err(e),
    }
}

/// The services the release did not just start, split into those whose
/// running copies differ from what this deploy starts and those it keeps.
fn split_changed<'a>(
    ctx: &'a ContainerApp,
    prepared: &Prepared,
) -> Result<(Vec<&'a ServiceDef>, Vec<&'a ServiceDef>)> {
    let candidates: Vec<&ServiceDef> = ctx
        .project
        .services
        .iter()
        .filter(|s| !prepared.started.contains(&s.name))
        .collect();
    let changed = match changed_services(ctx, &prepared.counts, candidates.clone()) {
        Ok(changed) => changed,
        Err(e) => return Err(e),
    };
    let kept = candidates
        .into_iter()
        .filter(|s| !changed.iter().any(|c| c.name == s.name))
        .collect();
    Ok((changed, kept))
}

/// Says each kept service is unchanged and publishes a kept routed
/// service's backends again, so routing setting changes still apply.
fn keep_services(ctx: &ContainerApp, kept: &[&ServiceDef]) -> Result<()> {
    for service in kept {
        display::step_done(&format!("Keeping {} (unchanged)", service.name), "");
        if let Err(e) = republish_routes(ctx, service) {
            return Err(e);
        }
    }
    Ok(())
}

/// Replace the copies of every service that changed, keep the others,
/// remove containers of dropped services, and take an app with no routed
/// port off the router.
fn roll_out_app(ctx: &ContainerApp, prepared: &Prepared) -> Result<()> {
    let (changed, kept) = match split_changed(ctx, prepared) {
        Ok(split) => split,
        Err(e) => return Err(e),
    };
    if let Err(e) = roll_out(ctx, &prepared.counts, &changed) {
        return Err(e);
    }
    if let Err(e) = keep_services(ctx, &kept) {
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
/// built. Only services whose running copies differ from what the deploy
/// starts (see [`super::changes`]) are replaced, in start order, one copy at
/// a time; the others keep running. A service with `service_healthy`
/// dependencies starts only once their copies are healthy. A failure stops the deploy and leaves the copies not yet
/// replaced serving. Containers of services the compose file no longer has
/// are removed, and an app with no routed port is taken off the router. An app
/// that ran as a Procfile app has its old workers stopped and deleted once the
/// containers are healthy and routed (see [`super::retire`]). The supervisor
/// is then signalled so it lists the new copies at once.
///
/// Before the release command runs, the services its service depends on are
/// started when none of their copies is up; those are not replaced again.
/// `replacing` is called once the images and the release command are ready,
/// right before the first copy is replaced. A failure before that call left
/// every running copy as it was. The release command's output goes on the
/// rail and its last lines into `recent`.
///
/// Blocks for as long as builds and health waits take, so it must not run on
/// a tokio runtime thread.
pub fn deploy_containers(
    app: &AppName,
    paths: &RikuPaths,
    env: &HashMap<String, String>,
    recent: &OutputTail,
    replacing: impl FnOnce(),
) -> Result<()> {
    let ctx = match ContainerApp::load(app, paths, env.clone()) {
        Ok(ctx) => ctx,
        Err(e) => return Err(e),
    };
    let prepared = match prepare(&ctx, recent) {
        Ok(prepared) => prepared,
        Err(e) => {
            // The release may have started services it depends on.
            notify_supervisor_reload(paths);
            return Err(e);
        }
    };
    let stale = match ProcfileWorkers::load(app.as_str(), paths) {
        Ok(stale) => stale,
        Err(e) => return Err(e),
    };
    replacing();
    let rolled = roll_out_app(&ctx, &prepared);
    notify_supervisor_reload(paths);
    match rolled {
        Ok(()) => retire_procfile_workers(&ctx, stale),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
#[path = "deploy_tests.rs"]
mod tests;
