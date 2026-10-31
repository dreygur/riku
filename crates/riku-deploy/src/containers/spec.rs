//! Building the engine run spec for one copy of a service.
//!
//! # Security model
//!
//! The compose parser already refuses the labels riku sets itself (app,
//! service, copy, port, restart). The compose project label is dropped here,
//! so a service cannot join another project's `list_project` results. App env
//! entries named `RIKU_*` configure riku itself (registry credentials, for
//! one) and are not passed on.

use std::collections::BTreeMap;

use anyhow::{anyhow, Result};
use riku_container::compose::{routed_http, ComposeProject, RoutedPort, ServiceDef};
use riku_container::engine::RunSpec;
use riku_container::types::{
    labels, ContainerName, CopyIndex, NetworkName, PortBinding, RestartPolicy,
};

use super::images::build_tag;
use super::AppName;

const RIKU_ENV_PREFIX: &str = "RIKU_";

fn restart_label(policy: RestartPolicy) -> String {
    match policy {
        RestartPolicy::No => "no".to_string(),
        RestartPolicy::Always => "always".to_string(),
        RestartPolicy::UnlessStopped => "unless-stopped".to_string(),
        RestartPolicy::OnFailure { max_retries: None } => "on-failure".to_string(),
        RestartPolicy::OnFailure {
            max_retries: Some(n),
        } => format!("on-failure:{n}"),
    }
}

fn is_routed(routed: &RoutedPort, service: &ServiceDef, binding: &PortBinding) -> bool {
    routed.service == service.name
        && routed.container_port == binding.container_port
        && routed.protocol == binding.protocol
        && routed.host_port == binding.host_port
}

fn bindings_for_copy(
    service: &ServiceDef,
    routed: Option<&RoutedPort>,
    host_port: Option<u16>,
) -> Vec<PortBinding> {
    let mut bindings = service.ports.clone();
    let (Some(routed), Some(host_port)) = (routed, host_port) else {
        return bindings;
    };
    if let Some(binding) = bindings.iter_mut().find(|b| is_routed(routed, service, b)) {
        binding.host_port = Some(host_port);
    }
    bindings
}

fn copy_labels(
    app: &AppName,
    service: &ServiceDef,
    copy: CopyIndex,
    published: Option<u16>,
) -> BTreeMap<String, String> {
    let mut labels_out = service.labels.clone();
    labels_out.remove(labels::COMPOSE_PROJECT);
    labels_out.insert(labels::APP.to_string(), app.to_string());
    labels_out.insert(labels::SERVICE.to_string(), service.name.to_string());
    labels_out.insert(labels::COPY.to_string(), copy.to_string());
    labels_out.insert(labels::RESTART.to_string(), restart_label(service.restart));
    if let Some(port) = published {
        labels_out.insert(labels::PORT.to_string(), port.to_string());
    }
    labels_out
}

fn copy_env(app_env: &BTreeMap<String, String>, service: &ServiceDef) -> BTreeMap<String, String> {
    let mut env: BTreeMap<String, String> = app_env
        .iter()
        .filter(|(name, _)| !name.starts_with(RIKU_ENV_PREFIX))
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect();
    env.extend(service.environment.clone());
    env
}

/// The run spec for copy `copy` of `service`.
///
/// `host_port` replaces the host port of the routed binding so every copy of
/// the routed service listens on its own port; it is ignored for other
/// services. The container env is the app env without `RIKU_*` entries,
/// overlaid by the service's own `environment`. A service with a `build` runs
/// the image tagged `riku-<app>-<service>`, otherwise its `image`.
pub fn copy_spec(
    app: &AppName,
    project: &ComposeProject,
    service: &ServiceDef,
    copy: CopyIndex,
    host_port: Option<u16>,
    env: &BTreeMap<String, String>,
) -> Result<RunSpec> {
    let name = match ContainerName::new(app.as_str(), &service.name, copy) {
        Ok(name) => name,
        Err(e) => return Err(e),
    };
    let image = match (&service.build, &service.image) {
        (Some(_), _) => build_tag(app, &service.name),
        (None, Some(image)) => image.clone(),
        (None, None) => {
            return Err(anyhow!(
                "service '{}' has neither image nor build",
                service.name
            ))
        }
    };
    let mounts = match project.engine_mounts(service) {
        Ok(mounts) => mounts,
        Err(e) => return Err(e),
    };
    let routed = match routed_http(project) {
        Ok(routed) => routed,
        Err(e) => return Err(e),
    };
    let ports = bindings_for_copy(service, routed.as_ref(), host_port);
    let published = routed
        .as_ref()
        .filter(|r| r.service == service.name)
        .and(host_port);
    Ok(RunSpec {
        name,
        image,
        network: NetworkName::for_app(app.as_str()),
        alias: service.name.clone(),
        labels: copy_labels(app, service, copy, published),
        env: copy_env(env, service),
        ports,
        mounts,
        entrypoint: service.entrypoint.clone(),
        command: service.command.clone(),
        working_dir: service.working_dir.clone(),
        user: service.user.clone(),
        stop_signal: service.stop_signal.clone(),
        stop_grace: service.stop_grace,
        healthcheck: service.healthcheck.clone(),
    })
}

#[cfg(test)]
#[path = "spec_tests.rs"]
mod tests;
