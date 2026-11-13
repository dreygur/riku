//! After a container deploy, put the app's copies into `stats.json` the way
//! the supervisor would, so `riku apps` and `riku ps` count them at once
//! instead of after the supervisor's next listing. Best effort: the
//! supervisor rewrites the rows from its own listing anyway, so a failure
//! here is logged and never fails the deploy.

use super::context::ContainerApp;
use super::removal::app_containers;
use crate::config::RikuPaths;
use crate::supervisor::containers::deployed_container_rows;
use crate::supervisor::stats::replace_app_container_rows;

/// Replace the app's container rows in `stats.json` with what the engine
/// reports for its containers now.
pub(super) fn publish_container_stats(ctx: &ContainerApp, paths: &RikuPaths) {
    let infos = match app_containers(&ctx.engine, ctx.app.as_str()) {
        Ok(infos) => infos,
        Err(e) => {
            tracing::warn!("cannot list {}'s containers for stats: {e:#}", ctx.app);
            return;
        }
    };
    let stats = paths.riku_root.join("stats.json");
    let rows = deployed_container_rows(&infos);
    if let Err(e) = replace_app_container_rows(&stats, ctx.app.as_str(), rows) {
        tracing::warn!("cannot update {}: {e:#}", stats.display());
    }
}
