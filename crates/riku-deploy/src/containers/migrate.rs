//! Moving an app from the old container plugin to riku managed containers.
//!
//! The new images are built and pulled while the old stack still serves.
//! The outage runs from stopping the old worker and its containers until the
//! new routed copy is healthy. Named volumes are kept and reused under the
//! names compose gave them (`<project>_<key>`); no path here removes a
//! volume. When the new containers fail, they are removed and the old worker
//! and its route come back. The project name was confirmed before this runs (see
//! [`super::project_check`]), so the volumes mounted are this app's own.

use std::collections::BTreeMap;
use std::time::{Duration, SystemTime};

use anyhow::{anyhow, Result};
use riku_container::compose::ComposeProject;
use riku_container::types::DEFAULT_STOP_GRACE;

use super::context::ContainerApp;
use super::deploy::roll_out_app;
use super::legacy_workers::LegacyWorkers;
use super::removal::{app_containers, remove_containers};
use super::rollback_wait::wait_restored_workers;
use crate::util::echo;

/// The old compose names containers `<project>-<service>-<n>`.
fn service_grace(project: &ComposeProject, container: &str) -> Duration {
    let service = container
        .strip_prefix(&format!("{}-", project.project_name()))
        .and_then(|rest| rest.rsplit_once('-'))
        .filter(|(_, index)| !index.is_empty() && index.bytes().all(|b| b.is_ascii_digit()))
        .map(|(service, _)| service);
    project
        .services
        .iter()
        .find(|s| Some(s.name.as_str()) == service)
        .map(|s| s.stop_grace)
        .unwrap_or(DEFAULT_STOP_GRACE)
}

fn stop_old_stack(ctx: &ContainerApp) -> Result<()> {
    let project = ctx.project.project_name();
    let ids = match ctx.engine.list_project(project) {
        Ok(ids) => ids,
        Err(e) => return Err(e),
    };
    let containers = match ctx.engine.inspect(&ids) {
        Ok(containers) => containers,
        Err(e) => return Err(e),
    };
    for info in &containers {
        if let Err(e) = ctx
            .engine
            .stop(&info.id, service_grace(&ctx.project, &info.name))
        {
            return Err(anyhow!("cannot stop old container {}: {e}", info.name));
        }
        if let Err(e) = ctx.engine.remove(&info.id) {
            return Err(anyhow!("cannot remove old container {}: {e}", info.name));
        }
    }
    let network = format!("{project}_default");
    match ctx.engine.remove_network(&network) {
        Ok(()) => Ok(()),
        Err(e) => Err(anyhow!("cannot remove old network {network}: {e}")),
    }
}

fn remove_new_copies(ctx: &ContainerApp) -> Result<()> {
    match app_containers(&ctx.engine, ctx.app.as_str()) {
        Ok(containers) => remove_containers(&ctx.engine, Some(&ctx.project), &containers),
        Err(e) => Err(e),
    }
}

fn republish_old_route(ctx: &ContainerApp) -> Result<()> {
    crate::router::configure(ctx.app.as_str(), &ctx.app_path, &ctx.env, ctx.paths, None)
}

/// Undo a failed migration. The old stack only comes back once riku's new
/// copies are gone, since both would mount the same volumes; otherwise the
/// workers stay parked and the next deploy picks the migration up again.
fn roll_back(ctx: &ContainerApp, legacy: &LegacyWorkers, failure: anyhow::Error) -> Result<()> {
    let app = ctx.app;
    if let Err(e) = remove_new_copies(ctx) {
        return Err(failure.context(format!(
            "migration of '{app}' failed and its new containers could not be removed ({e:#}); the old stack was left stopped"
        )));
    }
    if let Err(e) = legacy.restore(ctx.paths) {
        return Err(failure.context(format!(
            "migration of '{app}' failed and the old workers could not be restored ({e:#})"
        )));
    }
    if let Err(e) = wait_restored_workers(ctx.app.as_str(), ctx.paths) {
        return Err(failure.context(format!(
            "migration of '{app}' failed and the restored old workers did not answer ({e:#}); the old route was not republished"
        )));
    }
    if let Err(e) = republish_old_route(ctx) {
        return Err(failure.context(format!(
            "migration of '{app}' failed and the old route could not be republished ({e:#})"
        )));
    }
    Err(failure.context(format!(
        "migration of '{app}' failed; the old stack was restored"
    )))
}

/// Replace the old plugin's stack of `ctx.app` with riku containers, after
/// its images are ready: park the legacy workers, wait for the supervisor to
/// stop them, stop and remove the old containers and network, then roll out.
pub(crate) fn migrate(
    ctx: &ContainerApp,
    legacy: LegacyWorkers,
    counts: &BTreeMap<String, u32>,
) -> Result<()> {
    echo(
        &format!("-----> Migrating '{}' to riku containers; the app is down until its new copies are healthy", ctx.app),
        "yellow",
    );
    let parked_at = SystemTime::now();
    if let Err(e) = legacy.park(ctx.paths) {
        return roll_back(ctx, &legacy, e);
    }
    if let Err(e) = legacy.wait_stopped(ctx.paths, parked_at) {
        return roll_back(ctx, &legacy, e);
    }
    if let Err(e) = stop_old_stack(ctx) {
        return roll_back(ctx, &legacy, e);
    }
    if let Err(e) = roll_out_app(ctx, counts) {
        return roll_back(ctx, &legacy, e);
    }
    if let Err(e) = legacy.discard(ctx.paths) {
        tracing::warn!(app = %ctx.app, "migrated, but the old worker files remain: {e:#}");
    }
    echo(
        &format!("-----> Migrated '{}'. Volumes are kept.", ctx.app),
        "green",
    );
    Ok(())
}

#[cfg(test)]
#[path = "migrate_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "migrate_fix_tests.rs"]
mod fix_tests;
