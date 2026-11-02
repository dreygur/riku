//! Running the Procfile `release` command in a one-off container from the
//! newly built image, before any copy is replaced.
//!
//! # Security model
//!
//! The release line is one argv element handed to `/bin/sh -c` inside the
//! container, never to a shell on the host. App env reaches the engine only
//! through the env file in the 0700 scratch directory, which the engine
//! driver deletes before `Engine::run` returns. The container carries no riku
//! app, service or copy labels, so rollout, removal and listings never take
//! it for a copy.

use std::collections::BTreeMap;

use anyhow::{anyhow, Result};
use riku_container::compose::ServiceDef;
use riku_container::engine::{ContainerId, RunSpec};
use riku_container::types::{labels, ContainerName, CopyIndex};

use super::context::ContainerApp;
use super::copy_spec;
use crate::util::{echo, parse_procfile};

/// The routed service, or else the first service riku builds.
fn release_service<'a>(ctx: &'a ContainerApp<'a>) -> Result<&'a ServiceDef> {
    if let Some(routed) = &ctx.routed {
        return ctx.service(&routed.service);
    }
    match ctx.project.services.iter().find(|s| s.build.is_some()) {
        Some(service) => Ok(service),
        None => Err(anyhow!(
            "a release command needs a routed service or a service with a build section"
        )),
    }
}

fn release_labels(service: &ServiceDef) -> BTreeMap<String, String> {
    let mut labels_out = service.labels.clone();
    for key in [
        labels::COMPOSE_PROJECT,
        labels::APP,
        labels::SERVICE,
        labels::COPY,
        labels::PORT,
        labels::RESTART,
    ] {
        labels_out.remove(key);
    }
    labels_out
}

/// A copy's spec for `service` with the release line as its only process:
/// same image, env, network and mounts, but no ports and no health check.
fn release_spec(ctx: &ContainerApp, service: &ServiceDef, release: &str) -> Result<RunSpec> {
    let first = match CopyIndex::new(1) {
        Some(copy) => copy,
        None => return Err(anyhow!("copy index 1 was refused")),
    };
    let mut spec = match copy_spec(
        ctx.app,
        &ctx.project,
        service,
        first,
        None,
        &ctx.container_env,
    ) {
        Ok(spec) => spec,
        Err(e) => return Err(e),
    };
    spec.name = match ContainerName::for_release(ctx.app.as_str()) {
        Ok(name) => name,
        Err(e) => return Err(e),
    };
    spec.ports = Vec::new();
    spec.healthcheck = None;
    spec.entrypoint = Some(vec![
        "/bin/sh".to_string(),
        "-c".to_string(),
        release.to_string(),
    ]);
    spec.command = None;
    spec.labels = release_labels(service);
    Ok(spec)
}

/// Remove a release container a crashed deploy left behind.
fn remove_stale(ctx: &ContainerApp, name: &ContainerName) -> Result<()> {
    match ctx.engine.id_of(name) {
        Ok(Some(id)) => ctx
            .engine
            .remove(&id)
            .map_err(|e| anyhow!("cannot remove stale {}: {e}", name.as_str())),
        Ok(None) => Ok(()),
        Err(e) => Err(e),
    }
}

fn print_logs(ctx: &ContainerApp, id: &ContainerId) {
    if let Err(e) = ctx.engine.print_logs(id) {
        tracing::warn!("cannot show the release output: {e}");
    }
}

/// Start the release container, wait for it, show its output and remove it,
/// whatever the exit code.
fn run_to_exit(ctx: &ContainerApp, spec: &RunSpec) -> Result<()> {
    let id = match ctx.engine.run(spec, ctx.scratch()) {
        Ok(id) => id,
        Err(e) => return Err(anyhow!("cannot start the release container: {e}")),
    };
    let waited = ctx.engine.wait(&id);
    print_logs(ctx, &id);
    let removed = ctx.engine.remove(&id);
    match waited {
        Ok(0) => {}
        Ok(code) => return Err(anyhow!("release command exited with code {code}")),
        Err(e) => return Err(anyhow!("cannot wait for the release container: {e}")),
    }
    removed.map_err(|e| anyhow!("cannot remove the release container: {e}"))
}

/// Run the Procfile `release` command, when there is one, in a one-off
/// container from the new image. A nonzero exit is an error.
pub(super) fn run_release(ctx: &ContainerApp) -> Result<()> {
    let procfile = match parse_procfile(&ctx.app_path.join("Procfile")) {
        Ok(procfile) => procfile.unwrap_or_default(),
        Err(e) => return Err(anyhow!("invalid Procfile: {e}")),
    };
    let release = match procfile.get("release") {
        Some(release) => release,
        None => return Ok(()),
    };
    let service = match release_service(ctx) {
        Ok(service) => service,
        Err(e) => return Err(e),
    };
    let spec = match release_spec(ctx, service, release) {
        Ok(spec) => spec,
        Err(e) => return Err(e),
    };
    if let Err(e) = remove_stale(ctx, &spec.name) {
        return Err(e);
    }
    if let Err(e) = ctx.engine.ensure_network(&spec.network) {
        return Err(anyhow!(
            "cannot create network {}: {e}",
            spec.network.as_str()
        ));
    }
    echo("-----> Running release command", "green");
    run_to_exit(ctx, &spec)
}
