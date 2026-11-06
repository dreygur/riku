//! Single service image update for container apps.
//!
//! Called by the supervisor's periodic image watch. For riku container apps
//! it pulls the service image, compares engine image ids and replaces only
//! that service's copies when they differ.

// Project rule 16 forbids `?`, and this lint asks for exactly that rewrite.
#![allow(clippy::question_mark)]

use anyhow::{anyhow, Result};

use crate::config::RikuPaths;
use crate::containers::{is_built_service, AppName, ImageUpdate, ServiceName};

fn container_pull(app: &str, paths: &RikuPaths, service: &str) -> Result<()> {
    let app_name = match AppName::try_from(app) {
        Ok(name) => name,
        Err(e) => return Err(e),
    };
    let service_name = match ServiceName::try_from(service) {
        Ok(name) => name,
        Err(e) => return Err(e),
    };
    let is_built = match is_built_service(&app_name, paths, &service_name) {
        Ok(is_built) => is_built,
        Err(e) => return Err(e),
    };
    if is_built {
        tracing::info!(app, service, outcome = ?ImageUpdate::Built, "image watch check");
        return Ok(());
    }
    let update = match crate::containers::update_service(&app_name, paths, &service_name) {
        Ok(update) => update,
        Err(e) => return Err(e),
    };
    tracing::info!(app, service, outcome = ?update, "image watch check");
    Ok(())
}

/// Pull a fresh image for `service` and update that service.
///
/// Fails for an app riku does not run as containers, since only container
/// apps have service images to watch.
pub fn pull_service(app: &str, paths: &RikuPaths, service: &str) -> Result<()> {
    match crate::containers::runs_containers(app, paths) {
        Ok(true) => container_pull(app, paths, service),
        Ok(false) => Err(anyhow!(
            "'{app}' is not a container app; RIKU_WATCH_SERVICES has no effect on it"
        )),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
#[path = "service_update_tests.rs"]
mod tests;
