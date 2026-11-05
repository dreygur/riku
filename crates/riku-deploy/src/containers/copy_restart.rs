//! Restarting one dead container copy for the supervisor.

use anyhow::Result;
use riku_container::engine::{ContainerInfo, ContainerState};
use riku_container::types::{CopyIndex, ServiceName};
pub use riku_supervisor::health::RestartOutcome;

use super::context::{saved_env, ContainerApp};
use super::removal::{app_containers, remove_containers};
use super::AppName;
use crate::config::RikuPaths;
use crate::error::DeployError;
use crate::rolling::CopyRuntime;

fn is_busy(e: &anyhow::Error) -> bool {
    matches!(
        e.downcast_ref::<DeployError>(),
        Some(DeployError::DeployInProgress(_))
    )
}

fn reconfigure_router(ctx: &ContainerApp, service: &ServiceName) -> Result<()> {
    if !ctx.is_routed(service) {
        return Ok(());
    }
    crate::router::configure(
        ctx.app.as_str(),
        &ctx.app_path,
        &ctx.env,
        ctx.paths,
        Some(&ctx.engine),
    )
}

/// The dead copy is already gone, so before reporting `err` point the router
/// at the copies that are left, or take the app off the router when none
/// are. Both errors are kept when that fails too.
fn reroute_after_failure(
    ctx: &ContainerApp,
    service: &ServiceName,
    runtime: &impl CopyRuntime,
    err: anyhow::Error,
) -> anyhow::Error {
    if !ctx.is_routed(service) {
        return err;
    }
    let rerouted = match runtime.live() {
        Ok(left) if left.is_empty() => crate::router::remove(ctx.app.as_str(), ctx.paths),
        Ok(_) => reconfigure_router(ctx, service),
        Err(e) => Err(e),
    };
    match rerouted {
        Ok(()) => err,
        Err(e) => err.context(format!(
            "the router may still point at the removed copy: {e:#}"
        )),
    }
}

/// The exited container in slot `copy` of `service`, if the slot still has
/// one. The supervisor saw it die before taking the lock, so by now a stop, a
/// scale down or a deploy may have removed it or put a running copy there.
fn dead_copy(
    ctx: &ContainerApp,
    service: &ServiceName,
    copy: CopyIndex,
) -> Result<Option<ContainerInfo>> {
    let containers = match app_containers(&ctx.engine, ctx.app.as_str()) {
        Ok(containers) => containers,
        Err(e) => return Err(e),
    };
    Ok(containers.into_iter().find(|info| {
        info.service.as_ref() == Some(service)
            && info.copy == Some(copy)
            && matches!(info.state, ContainerState::Exited { .. })
    }))
}

fn start_slot(
    ctx: &ContainerApp,
    service: &ServiceName,
    copy: CopyIndex,
) -> Result<RestartOutcome> {
    let def = match ctx.service(service) {
        Ok(def) => def,
        Err(e) => return Err(e),
    };
    let dead = match dead_copy(ctx, service, copy) {
        Ok(Some(dead)) => dead,
        Ok(None) => return Ok(RestartOutcome::Gone),
        Err(e) => return Err(e),
    };
    if let Err(e) = remove_containers(&ctx.engine, Some(&ctx.project), &[dead]) {
        return Err(e.context(format!("cannot remove the dead copy {service}.{copy}")));
    }
    let mut runtime = ctx.runtime(def);
    let fresh = match runtime.start(copy) {
        Ok(fresh) => fresh,
        Err(e) => return Err(reroute_after_failure(ctx, service, &runtime, e)),
    };
    if let Err(e) = runtime.wait_healthy(&fresh) {
        runtime.discard(&fresh);
        let unhealthy = e.context(format!("{service}.{copy} did not become healthy"));
        return Err(reroute_after_failure(ctx, service, &runtime, unhealthy));
    }
    match reconfigure_router(ctx, service) {
        Ok(()) => Ok(RestartOutcome::Restarted),
        Err(e) => Err(e),
    }
}

/// Remove the dead copy `copy` of `service` and start the same slot again,
/// then point the router at the running copies when the service is routed.
///
/// Returns [`RestartOutcome::Busy`] without touching the engine when a deploy
/// or another operation holds the app's lock; the supervisor retries later.
/// Returns [`RestartOutcome::Gone`] without starting anything when the slot
/// no longer holds an exited container, so a `riku stop` that removed the
/// copy while the supervisor waited for the lock is not undone.
/// Blocks through the health wait, so it must not run on a tokio thread.
pub fn restart_copy(
    app: &AppName,
    paths: &RikuPaths,
    service: &ServiceName,
    copy: CopyIndex,
) -> Result<RestartOutcome> {
    let _lock = match crate::lock::acquire_for_restart(app.as_str(), paths) {
        Ok(lock) => lock,
        Err(e) if is_busy(&e) => return Ok(RestartOutcome::Busy),
        Err(e) => return Err(e),
    };
    let env = match saved_env(app.as_str(), paths) {
        Ok(env) => env,
        Err(e) => return Err(e),
    };
    let ctx = match ContainerApp::load(app, paths, env) {
        Ok(ctx) => ctx,
        Err(e) => return Err(e),
    };
    start_slot(&ctx, service, copy)
}
