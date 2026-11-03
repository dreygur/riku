//! Stopping and removing the Procfile workers of an app that has switched to
//! containers.

use std::time::SystemTime;

use anyhow::{anyhow, Result};

use super::context::ContainerApp;
use super::legacy_workers::LegacyWorkers;
use crate::util::echo;

fn still_running(app: &str, cause: anyhow::Error) -> anyhow::Error {
    anyhow!("the containers of '{app}' are up and routed, but its old workers could not be stopped: {cause:#}")
}

/// Park the old workers, wait until the supervisor has stopped them, then
/// delete their configs. Runs only after the containers are healthy and
/// routed, so a failed rollout leaves the Procfile app as it was.
pub(crate) fn retire_procfile_workers(ctx: &ContainerApp, workers: LegacyWorkers) -> Result<()> {
    if workers.is_empty() {
        return Ok(());
    }
    echo(
        &format!("-----> Stopping the old Procfile workers of '{}'", ctx.app),
        "green",
    );
    let parked_at = SystemTime::now();
    if let Err(e) = workers.park(ctx.paths) {
        return Err(still_running(ctx.app.as_str(), e));
    }
    if let Err(e) = workers.wait_stopped(ctx.paths, parked_at) {
        return Err(still_running(ctx.app.as_str(), e));
    }
    if let Err(e) = workers.discard(ctx.paths) {
        tracing::warn!(app = %ctx.app, "old workers stopped, but their files remain: {e:#}");
    }
    Ok(())
}
