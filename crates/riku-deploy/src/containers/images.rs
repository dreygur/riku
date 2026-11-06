//! Making the images of a project available to the engine.
//!
//! The registry token is read from the app env and handed to
//! `Engine::login`, which writes it to the engine's stdin. It is not logged
//! and not passed to builds. Build args come from the compose file only and
//! are visible in the process list while a build runs.
//!
//! `Engine::login` makes the engine store the credentials in its user's shared
//! auth file. They outlive the removal of the ENV keys, apply to every app on
//! the server, and the last login wins. An app owner who interpolates
//! `RIKU_GHCR_TOKEN` into compose build args or environment puts the token
//! there themselves.

use std::collections::BTreeMap;

use anyhow::{anyhow, Result};
use riku_container::compose::{ComposeProject, ServiceDef};
use riku_container::engine::{BuildSpec, Engine};
use riku_container::types::ServiceName;
use sha2::{Digest, Sha256};

use super::AppName;

const GHCR_REGISTRY: &str = "ghcr.io";
const GHCR_USERNAME: &str = "RIKU_GHCR_USERNAME";
const GHCR_TOKEN: &str = "RIKU_GHCR_TOKEN";

/// The local tag a built service gets: `localhost/riku-<h>`, where `<h>` is the
/// first 16 hex characters of the SHA-256 of the app name, a NUL byte and the
/// service name.
///
/// The `localhost/` host makes the reference resolve only to a local registry
/// (and is podman's own prefix for local images). A bare `riku/<h>` would be a
/// Docker Hub repository, so a missing local image could be pulled from
/// whoever published that name. Runs of built images also use
/// `PullPolicy::Never`.
///
/// A readable `riku-<app>-<service>` is ambiguous: `a-b`/`c` and `a`/`b-c`
/// join to the same text, and lowercasing for the engine merges `Shop` with
/// `shop`. Two apps sharing a tag would let one build replace the other's
/// image. NUL cannot occur in either name, so the hash input is unambiguous.
pub(super) fn build_tag(app: &AppName, service: &ServiceName) -> String {
    let mut hasher = Sha256::new();
    hasher.update(app.as_str().as_bytes());
    hasher.update([0u8]);
    hasher.update(service.as_str().as_bytes());
    let digest = hex::encode(hasher.finalize());
    format!("localhost/riku-{}", &digest[..16])
}

fn login_if_configured(engine: &Engine, env: &BTreeMap<String, String>) -> Result<()> {
    match (env.get(GHCR_USERNAME), env.get(GHCR_TOKEN)) {
        (Some(user), Some(token)) => engine.login(GHCR_REGISTRY, user, token),
        (None, None) => Ok(()),
        _ => Err(anyhow!(
            "set both {GHCR_USERNAME} and {GHCR_TOKEN} to log in to {GHCR_REGISTRY}, or neither"
        )),
    }
}

/// Log in to the registry if configured and pull the image.
pub(super) fn pull_image(
    engine: &Engine,
    env: &BTreeMap<String, String>,
    image: &str,
) -> Result<()> {
    if let Err(e) = login_if_configured(engine, env) {
        return Err(e);
    }
    engine.pull(image)
}

fn prepare_service(engine: &Engine, app: &AppName, service: &ServiceDef) -> Result<()> {
    if let Some(build) = &service.build {
        let spec = BuildSpec {
            context: build.context.clone(),
            dockerfile: build.dockerfile.clone(),
            tag: build_tag(app, &service.name),
            args: build.args.clone(),
            platform: None,
        };
        return engine.build(&spec);
    }
    match &service.image {
        Some(image) => engine.pull(image),
        None => Err(anyhow!(
            "service '{}' has neither image nor build",
            service.name
        )),
    }
}

/// Log in to the registry when credentials are set, pull the `image`
/// services and build the `build` services. A service with both is built.
pub fn prepare_images(
    engine: &Engine,
    app: &AppName,
    project: &ComposeProject,
    env: &BTreeMap<String, String>,
) -> Result<()> {
    if let Err(e) = login_if_configured(engine, env) {
        return Err(e);
    }
    for service in &project.services {
        if let Err(e) = prepare_service(engine, app, service) {
            return Err(anyhow!("service '{}': {e}", service.name));
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "images_tests.rs"]
mod tests;
