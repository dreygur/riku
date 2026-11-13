//! Which services a deploy replaces.
//!
//! A service is left running when every one of its copies is up, there are
//! as many as `SCALING` wants, each carries the `riku.spec` fingerprint the
//! deploy would start it with, and each runs the image id its image
//! reference names now. Anything else, a container started before riku set
//! the label included, is replaced as before.

use std::collections::BTreeMap;

use anyhow::{anyhow, Result};
use riku_container::compose::ServiceDef;
use riku_container::engine::{ContainerInfo, ContainerState};

use super::app::wanted;
use super::context::ContainerApp;
use super::images::build_tag;
use super::removal::app_containers;
use super::spec::spec_fingerprint;

/// Docker reports ids as `sha256:<hex>`; other engines may leave the
/// prefix off in one listing and not the other (podman 5 prints it in
/// `image ls` but not in a container's `Image`).
pub(super) fn bare_id(id: &str) -> &str {
    id.trim().trim_start_matches("sha256:")
}

fn image_ref(ctx: &ContainerApp, service: &ServiceDef) -> Result<String> {
    match (&service.build, &service.image) {
        (Some(_), _) => Ok(build_tag(ctx.app, &service.name)),
        (None, Some(image)) => Ok(image.clone()),
        (None, None) => Err(anyhow!(
            "service '{}' has neither image nor build",
            service.name
        )),
    }
}

/// Whether the copies in `copies` are exactly what this deploy would start.
fn unchanged(
    ctx: &ContainerApp,
    service: &ServiceDef,
    want: u32,
    copies: &[&ContainerInfo],
) -> Result<bool> {
    if copies.len() != want as usize || copies.iter().any(|c| c.state != ContainerState::Running) {
        return Ok(false);
    }
    let fingerprint = match spec_fingerprint(ctx.app, &ctx.project, service, &ctx.container_env) {
        Ok(fingerprint) => fingerprint,
        Err(e) => return Err(e),
    };
    if copies
        .iter()
        .any(|c| c.spec.as_deref() != Some(fingerprint.as_str()))
    {
        return Ok(false);
    }
    let image = match image_ref(ctx, service) {
        Ok(image) => image,
        Err(e) => return Err(e),
    };
    // An image the lookup cannot pin to one id is replaced, as every
    // service was before this check existed, rather than failing the deploy.
    let image_id = match ctx.engine.image_id(&image) {
        Ok(Some(id)) => id,
        Ok(None) => return Ok(false),
        Err(e) => {
            tracing::warn!(service = %service.name, "cannot compare the image: {e}");
            return Ok(false);
        }
    };
    Ok(copies
        .iter()
        .all(|c| bare_id(&c.image_id) == bare_id(&image_id)))
}

/// The services of `services`, in the order given, whose running copies
/// differ from what this deploy would start, at the copy counts `counts`
/// names.
pub(super) fn changed_services<'s>(
    ctx: &ContainerApp,
    counts: &BTreeMap<String, u32>,
    services: Vec<&'s ServiceDef>,
) -> Result<Vec<&'s ServiceDef>> {
    let containers = match app_containers(&ctx.engine, ctx.app.as_str()) {
        Ok(containers) => containers,
        Err(e) => return Err(e),
    };
    let mut changed = Vec::new();
    for service in services {
        let copies: Vec<&ContainerInfo> = containers
            .iter()
            .filter(|info| info.service.as_ref() == Some(&service.name))
            .collect();
        match unchanged(ctx, service, wanted(counts, &service.name), &copies) {
            Ok(true) => {}
            Ok(false) => changed.push(service),
            Err(e) => return Err(e),
        }
    }
    Ok(changed)
}

#[cfg(test)]
#[path = "changes_tests.rs"]
mod tests;
