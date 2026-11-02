//! Following a registry tag: replace a service's copies when the image the
//! tag names is no longer the one they run.
//!
//! # Security model
//!
//! The registry token only reaches the engine through `Engine::login` on
//! stdin. Image references and ids are not logged, only service names.

use anyhow::{anyhow, Result};
use riku_container::compose::ServiceDef;
use riku_container::engine::ContainerState;
use riku_container::types::ServiceName;

use super::context::ContainerApp;
use super::AppName;
use crate::config::RikuPaths;

/// What one image watch check did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageUpdate {
    /// Every running copy already runs the image the tag now names.
    Unchanged,
    /// The tag moved and the service copies were replaced.
    Replaced,
    /// The service has no running copies, so there was nothing to compare. Nothing was started.
    NotRunning,
    /// The service is built from source, there is nothing to pull, nothing was done.
    Built,
}

fn is_running(state: &ContainerState) -> bool {
    matches!(
        state,
        ContainerState::Running | ContainerState::Starting | ContainerState::Unhealthy
    )
}

fn watched_image(def: &ServiceDef) -> Result<Option<&str>> {
    if def.build.is_some() {
        return Ok(None);
    }
    match &def.image {
        Some(image) => Ok(Some(image.as_str())),
        None => Err(anyhow!("service '{}' has no image", def.name)),
    }
}

fn running_image_ids(ctx: &ContainerApp, service: &ServiceName) -> Result<Vec<String>> {
    let all = match super::removal::app_containers(&ctx.engine, ctx.app.as_str()) {
        Ok(containers) => containers,
        Err(e) => return Err(e),
    };
    let ids = all
        .into_iter()
        .filter(|info| info.service.as_ref() == Some(service) && is_running(&info.state))
        .map(|info| info.image_id)
        .collect();
    Ok(ids)
}

/// Pull the image for a service and replace its copies if the tag moved.
pub fn update_service(
    app: &AppName,
    paths: &RikuPaths,
    service: &ServiceName,
) -> Result<ImageUpdate> {
    let (_lock, ctx) = match super::control::locked(app, paths) {
        Ok(locked) => locked,
        Err(e) => return Err(e),
    };
    let def = match ctx.service(service) {
        Ok(def) => def,
        Err(e) => return Err(e),
    };
    let image = match watched_image(def) {
        Ok(Some(image)) => image,
        // Backstop for a compose file that changes between the pre-lock check and here.
        Ok(None) => return Ok(ImageUpdate::Built),
        Err(e) => return Err(e),
    };
    if let Err(e) = super::images::pull_image(&ctx.engine, &ctx.container_env, image) {
        return Err(e);
    }
    let pulled_id = match ctx.engine.image_id(image) {
        Ok(Some(id)) => id,
        Ok(None) => {
            return Err(anyhow!(
                "image of service '{service}' is not present after pull"
            ));
        }
        Err(e) => return Err(e),
    };
    let running = match running_image_ids(&ctx, service) {
        Ok(ids) => ids,
        Err(e) => return Err(e),
    };
    if running.is_empty() {
        return Ok(ImageUpdate::NotRunning);
    }
    if running.iter().all(|id| id == &pulled_id) {
        return Ok(ImageUpdate::Unchanged);
    }
    let counts = match super::app::read_scaling(app.as_str(), paths) {
        Ok(counts) => counts,
        Err(e) => return Err(e),
    };
    if let Err(e) = super::rollout::roll_out(&ctx, &counts, &[def]) {
        return Err(e);
    }
    Ok(ImageUpdate::Replaced)
}

/// Whether a service is built from source instead of pulled from a registry.
///
/// Reads only the app's env and compose file, without taking the deploy lock
/// or opening the container engine.
pub fn is_built_service(app: &AppName, paths: &RikuPaths, service: &ServiceName) -> Result<bool> {
    let env = match super::context::saved_env(app.as_str(), paths) {
        Ok(env) => env,
        Err(e) => return Err(e),
    };
    let app_path = paths.app_root.join(app.as_str());
    let project = match super::app::load_project(app.as_str(), &app_path, &env) {
        Ok(project) => project,
        Err(e) => return Err(e),
    };
    let def = match project.services.iter().find(|s| &s.name == service) {
        Some(def) => def,
        None => {
            return Err(anyhow!(
                "app '{app}' has no service '{service}' in its compose file"
            ));
        }
    };
    Ok(def.build.is_some())
}
