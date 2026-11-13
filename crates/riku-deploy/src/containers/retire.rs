//! Stopping and removing the Procfile workers of an app that has switched to
//! containers.

use anyhow::{anyhow, Result};

use super::app::retain_scaling;
use super::context::ContainerApp;
use super::procfile_workers::ProcfileWorkers;
use crate::util::display;

fn still_running(app: &str, cause: anyhow::Error) -> anyhow::Error {
    anyhow!("the containers of '{app}' are up and routed, but its old workers could not be stopped: {cause:#}")
}

/// Park the old workers, wait until the supervisor has stopped them, then
/// delete their configs. Runs only after the containers are healthy and
/// routed, so a failed rollout leaves the Procfile app as it was.
pub(crate) fn retire_procfile_workers(ctx: &ContainerApp, workers: ProcfileWorkers) -> Result<()> {
    if workers.is_empty() {
        return Ok(());
    }
    display::step_done(
        &format!("Stopping the old Procfile workers of '{}'", ctx.app),
        "",
    );
    if let Err(e) = workers.park(ctx.paths) {
        return Err(still_running(ctx.app.as_str(), e));
    }
    if let Err(e) = workers.wait_stopped(ctx.paths) {
        return Err(still_running(ctx.app.as_str(), e));
    }
    if let Err(e) = workers.discard(ctx.paths) {
        tracing::warn!(app = %ctx.app, "old workers stopped, but their files remain: {e:#}");
    }
    if let Err(e) = forget_procfile_scaling(ctx) {
        tracing::warn!(app = %ctx.app, "old workers stopped, but SCALING keeps their kinds: {e:#}");
    }
    Ok(())
}

/// Drop the `SCALING` entries of Procfile kinds the compose file has no
/// service for. Before the switch every entry was a Procfile kind.
fn forget_procfile_scaling(ctx: &ContainerApp) -> Result<()> {
    let is_service = |name: &str| {
        ctx.project
            .services
            .iter()
            .any(|service| service.name.as_str() == name)
    };
    retain_scaling(ctx.app.as_str(), ctx.paths, is_service)
}
