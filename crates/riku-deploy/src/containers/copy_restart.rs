//! Restarting one dead container copy for the supervisor.

use anyhow::Result;
use riku_container::types::{CopyIndex, ServiceName};
pub use riku_supervisor::health::RestartOutcome;

use super::context::{saved_env, ContainerApp};
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

fn start_slot(ctx: &ContainerApp, service: &ServiceName, copy: CopyIndex) -> Result<()> {
    let def = match ctx.service(service) {
        Ok(def) => def,
        Err(e) => return Err(e),
    };
    let mut runtime = ctx.runtime(def);
    let live = match runtime.live() {
        Ok(live) => live,
        Err(e) => return Err(e),
    };
    if let Some(dead) = live.iter().find(|c| c.copy == copy) {
        if let Err(e) = runtime.stop(dead) {
            return Err(e.context(format!("cannot remove the dead copy {service}.{copy}")));
        }
    }
    let fresh = match runtime.start(copy) {
        Ok(fresh) => fresh,
        Err(e) => return Err(reroute_after_failure(ctx, service, &runtime, e)),
    };
    if let Err(e) = runtime.wait_healthy(&fresh) {
        runtime.discard(&fresh);
        let unhealthy = e.context(format!("{service}.{copy} did not become healthy"));
        return Err(reroute_after_failure(ctx, service, &runtime, unhealthy));
    }
    reconfigure_router(ctx, service)
}

/// Remove the dead copy `copy` of `service` and start the same slot again,
/// then point the router at the running copies when the service is routed.
///
/// Returns [`RestartOutcome::Busy`] without touching the engine when a deploy
/// or another operation holds the app's lock; the supervisor retries later.
/// Blocks through the health wait, so it must not run on a tokio thread.
pub fn restart_copy(
    app: &AppName,
    paths: &RikuPaths,
    service: &ServiceName,
    copy: CopyIndex,
) -> Result<RestartOutcome> {
    let _lock = match crate::lock::acquire(app.as_str(), paths) {
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
    match start_slot(&ctx, service, copy) {
        Ok(()) => Ok(RestartOutcome::Restarted),
        Err(e) => Err(e),
    }
}
