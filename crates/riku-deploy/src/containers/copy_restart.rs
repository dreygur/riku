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
        Err(e) => return Err(e),
    };
    if let Err(e) = runtime.wait_healthy(&fresh) {
        runtime.discard(&fresh);
        return Err(e.context(format!("{service}.{copy} did not become healthy")));
    }
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
