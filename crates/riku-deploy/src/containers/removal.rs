//! Stopping and removing container copies outside a rolling replacement:
//! services dropped from the compose file, `riku stop` and `riku destroy`.
//!
//! Only containers, and on destroy the app network, are removed. Volumes are
//! never touched, so an app's data outlives its containers.

use std::time::Duration;

use anyhow::{anyhow, Result};
use riku_container::compose::ComposeProject;
use riku_container::engine::{ContainerInfo, Engine};
use riku_container::types::{NetworkName, DEFAULT_STOP_GRACE};

use super::app::{load_project, open_engine};
use super::context::saved_env;
use super::AppName;
use crate::config::RikuPaths;
use crate::util::echo;

/// The service's own stop grace, or the default for a container whose
/// service is no longer in `project` (or when there is no project).
fn grace_of(project: Option<&ComposeProject>, info: &ContainerInfo) -> Duration {
    project
        .and_then(|project| {
            project
                .services
                .iter()
                .find(|s| Some(&s.name) == info.service.as_ref())
        })
        .map(|service| service.stop_grace)
        .unwrap_or(DEFAULT_STOP_GRACE)
}

/// Every container of `app`, with its state.
pub(crate) fn app_containers(engine: &Engine, app: &str) -> Result<Vec<ContainerInfo>> {
    let ids = match engine.list(app) {
        Ok(ids) => ids,
        Err(e) => return Err(e),
    };
    engine.inspect(&ids)
}

/// Stop each container with its service's grace period, then remove it.
pub(crate) fn remove_containers(
    engine: &Engine,
    project: Option<&ComposeProject>,
    containers: &[ContainerInfo],
) -> Result<()> {
    for info in containers {
        if let Err(e) = engine.stop(&info.id, grace_of(project, info)) {
            return Err(anyhow!("cannot stop {}: {e}", info.name));
        }
        if let Err(e) = engine.remove(&info.id) {
            return Err(anyhow!("cannot remove {}: {e}", info.name));
        }
    }
    Ok(())
}

/// Remove the containers of services that are no longer in `project`.
pub(crate) fn remove_dropped(engine: &Engine, app: &str, project: &ComposeProject) -> Result<()> {
    let containers = match app_containers(engine, app) {
        Ok(containers) => containers,
        Err(e) => return Err(e),
    };
    let dropped: Vec<ContainerInfo> = containers
        .into_iter()
        .filter(|info| {
            !project
                .services
                .iter()
                .any(|s| Some(&s.name) == info.service.as_ref())
        })
        .collect();
    if dropped.is_empty() {
        return Ok(());
    }
    echo(
        &format!(
            "-----> Removing {} containers of services no longer in the compose file",
            dropped.len()
        ),
        "yellow",
    );
    remove_containers(engine, Some(project), &dropped)
}

/// Stop and remove every container of `app` and its network, for
/// `riku destroy`. Holds the deploy lock for the whole removal. Volumes are
/// kept, and the output says so.
///
/// Runs while the checkout still exists, so the services' grace periods are
/// used when the compose file can still be read.
pub fn destroy_containers(app: &AppName, paths: &RikuPaths) -> Result<()> {
    let _lock = match crate::lock::acquire(app.as_str(), paths) {
        Ok(lock) => lock,
        Err(e) => return Err(e),
    };
    let engine = match open_engine(paths) {
        Ok(engine) => engine,
        Err(e) => return Err(e),
    };
    let containers = match app_containers(&engine, app.as_str()) {
        Ok(containers) => containers,
        Err(e) => return Err(e),
    };
    let project = match saved_env(app.as_str(), paths) {
        Ok(env) => load_project(app.as_str(), &paths.app_root.join(app.as_str()), &env).ok(),
        Err(_) => None,
    };
    if let Err(e) = remove_containers(&engine, project.as_ref(), &containers) {
        return Err(e);
    }
    let network = NetworkName::for_app(app.as_str());
    if let Err(e) = engine.remove_network(network.as_str()) {
        return Err(anyhow!("cannot remove network {}: {e}", network.as_str()));
    }
    echo(
        &format!(
            "--> Removed {} containers and network {} of '{app}'. Volumes are kept.",
            containers.len(),
            network.as_str()
        ),
        "yellow",
    );
    Ok(())
}
