//! Rules a project must satisfy: which service is routed, which can scale and
//! how each is replaced during a deploy.

use super::{ComposeProject, ServiceDef};
use crate::types::{labels, Mount, MountSource, PortBinding, Protocol, ServiceName};
use anyhow::{anyhow, bail, Result};

/// The one port binding that carries the app's HTTP traffic.
///
/// `container_port`, `protocol` and `host_port` are copied from that binding
/// of the compose file, so a later step can tell it from any other binding of
/// the service and replace its host port per copy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutedPort {
    pub service: ServiceName,
    pub container_port: u16,
    pub protocol: Protocol,
    /// The host port the compose file gave, `None` when the engine picks it.
    pub host_port: Option<u16>,
}

impl RoutedPort {
    fn new(service: &ServiceDef, binding: &PortBinding) -> RoutedPort {
        RoutedPort {
            service: service.name.clone(),
            container_port: binding.container_port,
            protocol: binding.protocol,
            host_port: binding.host_port,
        }
    }

    fn is(&self, service: &ServiceName, binding: &PortBinding) -> bool {
        &self.service == service
            && self.container_port == binding.container_port
            && self.protocol == binding.protocol
            && self.host_port == binding.host_port
    }
}

/// How a service is swapped for its new version during a rolling deploy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplaceOrder {
    /// The new copy starts before the old one stops.
    Overlap,
    /// The old copy stops first, so a scheduled job never runs twice.
    StopFirst,
}

fn published(binding: &PortBinding) -> u16 {
    binding.host_port.unwrap_or(binding.container_port)
}

/// The service that serves the app's domains.
///
/// A service labelled `riku.http` wins, and the label value must be one of its
/// published host ports. With no label, routing works only when exactly one
/// service publishes exactly one port; no published port gives `None` and
/// anything else is refused.
pub fn routed_http(project: &ComposeProject) -> Result<Option<RoutedPort>> {
    let labelled: Vec<&ServiceDef> = project
        .services
        .iter()
        .filter(|service| service.labels.contains_key(labels::HTTP))
        .collect();
    match labelled.as_slice() {
        [] => route_without_label(project),
        [service] => route_from_label(service).map(Some),
        several => {
            let names: Vec<String> = several.iter().map(|s| s.name.to_string()).collect();
            bail!(
                "services {} all carry the label {}; only one service can be routed",
                names.join(", "),
                labels::HTTP
            )
        }
    }
}

fn route_from_label(service: &ServiceDef) -> Result<RoutedPort> {
    let value = &service.labels[labels::HTTP];
    let Ok(port) = value.parse::<u16>() else {
        bail!(
            "service '{}': label {} must be a port number, not {value:?}",
            service.name,
            labels::HTTP
        );
    };
    match service.ports.iter().find(|b| b.host_port == Some(port)) {
        Some(binding) => Ok(RoutedPort::new(service, binding)),
        None => bail!(
            "service '{}': label {} is {port}, which is not one of its published ports",
            service.name,
            labels::HTTP
        ),
    }
}

fn route_without_label(project: &ComposeProject) -> Result<Option<RoutedPort>> {
    let ports: Vec<(&ServiceDef, &PortBinding)> = project
        .services
        .iter()
        .flat_map(|service| service.ports.iter().map(move |binding| (service, binding)))
        .collect();
    match ports.as_slice() {
        [] => Ok(None),
        [(service, binding)] => Ok(Some(RoutedPort::new(service, binding))),
        several => {
            let listed: Vec<String> = several
                .iter()
                .map(|(service, binding)| format!("{}: {}", service.name, published(binding)))
                .collect();
            bail!(
                "several published ports ({}); add the label {}: \"<port>\" to the service that serves the app's domains",
                listed.join(", "),
                labels::HTTP
            )
        }
    }
}

fn protocol_name(protocol: Protocol) -> &'static str {
    match protocol {
        Protocol::Tcp => "tcp",
        Protocol::Udp => "udp",
    }
}

fn describe_mount(mount: &Mount) -> String {
    let target = mount.target.display();
    match &mount.source {
        MountSource::Volume(name) => format!("volume {name} at {target}"),
        MountSource::Bind(path) => format!("bind mount {} at {target}", path.display()),
        MountSource::Anonymous => format!("anonymous volume at {target}"),
    }
}

/// Check that `service` can run as `copies` copies.
///
/// One copy always passes. More are refused for a service that mounts writable
/// storage, publishes any port other than the routed HTTP one (routers only
/// balance HTTP). The routed service may carry a fixed host mapping such as
/// `"8080:80"`: phase 3 gives each copy its own host port on 127.0.0.1 and
/// lists every copy as a router backend.
pub fn check_scale(project: &ComposeProject, service: &ServiceName, copies: u32) -> Result<()> {
    if copies == 0 {
        bail!("service '{service}': copies must be at least 1");
    }
    let Some(def) = project.services.iter().find(|s| &s.name == service) else {
        return Err(anyhow!("service '{service}' is not in the compose file"));
    };
    if copies == 1 {
        return Ok(());
    }
    if let Some(mount) = def.mounts.iter().find(|m| m.is_writable()) {
        bail!(
            "service '{service}' mounts writable storage ({}); storage services cannot scale",
            describe_mount(mount)
        );
    }
    let routed = match routed_http(project) {
        Ok(routed) => routed,
        Err(e) => return Err(e),
    };
    for binding in &def.ports {
        let shares_container_port = def
            .ports
            .iter()
            .filter(|other| other.container_port == binding.container_port)
            .count()
            > 1;
        let is_routed = binding.protocol == Protocol::Tcp
            && !shares_container_port
            && routed.as_ref().is_some_and(|r| r.is(service, binding));
        if !is_routed {
            bail!(
                "service '{service}' publishes port {} ({}/{}), which is not routed HTTP; only routed HTTP services can scale",
                published(binding),
                binding.container_port,
                protocol_name(binding.protocol)
            );
        }
    }
    Ok(())
}

/// How `service` is replaced during a rolling deploy. Only the routed HTTP
/// service with no writable mount overlaps. When routing cannot be decided,
/// every service is stopped first.
pub fn replace_order(project: &ComposeProject, service: &ServiceName) -> ReplaceOrder {
    let is_routed = matches!(routed_http(project), Ok(Some(r)) if &r.service == service);
    let stateless = project
        .services
        .iter()
        .find(|s| &s.name == service)
        .is_some_and(|s| !s.mounts.iter().any(Mount::is_writable));
    if is_routed && stateless {
        ReplaceOrder::Overlap
    } else {
        ReplaceOrder::StopFirst
    }
}

#[cfg(test)]
#[path = "rules_tests.rs"]
mod tests;
