//! Starting one container copy.

use std::path::Path;

use anyhow::{anyhow, Result};
use riku_container::compose::check_bind_source;
use riku_container::engine::{ContainerId, Engine, RunSpec};
use riku_container::types::{labels, MountSource};

fn check_name_is_free(engine: &Engine, spec: &RunSpec) -> Result<()> {
    let app = match spec.labels.get(labels::APP) {
        Some(app) => app,
        None => {
            return Err(anyhow!(
                "{} has no {} label",
                spec.name.as_str(),
                labels::APP
            ))
        }
    };
    let owner = match engine.owner_of(&spec.name) {
        Ok(owner) => owner,
        Err(e) => return Err(e),
    };
    match owner {
        Some(holder) if &holder != app => Err(anyhow!(
            "container name {} is held by {}, so app `{app}` cannot start it",
            spec.name.as_str(),
            if holder.is_empty() {
                "a container riku does not manage".to_string()
            } else {
                format!("app `{holder}`")
            }
        )),
        _ => Ok(()),
    }
}

fn check_bind_sources(spec: &RunSpec, app_path: &Path) -> Result<()> {
    for mount in &spec.mounts {
        if let MountSource::Bind(source) = &mount.source {
            if let Err(e) = check_bind_source(app_path, source) {
                return Err(anyhow!("cannot start {}: {e}", spec.name.as_str()));
            }
        }
    }
    Ok(())
}

/// Start the container `spec` describes and return its id.
///
/// Refuses a name that another app (or a container riku did not create)
/// holds, and checks every bind source for symlinks again, because the
/// checkout may have changed since the compose file was parsed. `scratch`
/// receives the env file for the duration of the call.
pub fn start_copy(
    engine: &Engine,
    spec: &RunSpec,
    app_path: &Path,
    scratch: &Path,
) -> Result<ContainerId> {
    if let Err(e) = check_name_is_free(engine, spec) {
        return Err(e);
    }
    if let Err(e) = check_bind_sources(spec, app_path) {
        return Err(e);
    }
    if let Err(e) = engine.ensure_network(&spec.network) {
        return Err(anyhow!(
            "cannot prepare network {}: {e}",
            spec.network.as_str()
        ));
    }
    engine.run(spec, scratch)
}

#[cfg(test)]
#[path = "start_tests.rs"]
mod tests;
