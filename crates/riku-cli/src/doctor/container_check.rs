//! `riku doctor` check for the container engine.
//!
//! Read-only like every other doctor check: opening the engine only runs its
//! probe and `version` commands, and the buildx check runs
//! `docker buildx version`.

use crate::config::RikuPaths;
use crate::deploy::containers;

use super::Check;

const ENGINE_NAME: &str = "container engine";
const BUILDX_NAME: &str = "docker buildx";

/// The buildx line for an engine whose [`docker_has_buildx`] answer is
/// `has_buildx`; none for an engine other than docker.
///
/// [`docker_has_buildx`]: riku_container::engine::Engine::docker_has_buildx
fn buildx_check(has_buildx: Option<bool>) -> Option<Check> {
    match has_buildx {
        None => None,
        Some(true) => Some(Check::ok(BUILDX_NAME, "installed")),
        Some(false) => Some(Check::warn(
            BUILDX_NAME,
            "missing, so docker builds with its deprecated legacy builder. \
             Install the docker-buildx-plugin package from Docker's repository \
             (see https://docs.docker.com/go/buildx/)",
        )),
    }
}

/// Report the detected container engine and version, and for docker whether
/// buildx is installed, or warn if no engine can be opened.
pub fn container_engine(paths: &RikuPaths) -> Vec<Check> {
    match containers::open_engine(paths) {
        Ok(engine) => {
            let identity = engine.identity();
            let mut checks = vec![Check::ok(
                ENGINE_NAME,
                format!("{} {}", identity.name, identity.version),
            )];
            checks.extend(buildx_check(engine.docker_has_buildx()));
            checks
        }
        Err(e) => vec![Check::warn(ENGINE_NAME, format!("{e:#}"))],
    }
}

#[cfg(test)]
#[path = "container_check_tests.rs"]
mod tests;
