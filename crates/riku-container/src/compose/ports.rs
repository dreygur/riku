//! Published ports.
//!
//! Which interface a port binds is decided by the service's `riku.public`
//! label alone. A compose port that names a host IP is refused instead of
//! honoured, so a service cannot reach all interfaces without the label, and
//! the label accepts only `"true"` and `"false"`.

use super::scalar::reject_unknown;
use super::Value;
use crate::types::{labels, HostIp, PortBinding, Protocol};
use anyhow::{bail, Result};
use std::collections::BTreeMap;

const HOST_IP_REFUSAL: &str = "use the riku.public label instead of a host IP";

/// The interface for a service's ports: all interfaces only for
/// `riku.public: "true"`, loopback when the label is absent or `"false"`.
pub(super) fn host_ip_for(service_labels: &BTreeMap<String, String>) -> Result<HostIp> {
    match service_labels.get(labels::PUBLIC).map(String::as_str) {
        None | Some("false") => Ok(HostIp::Loopback),
        Some("true") => Ok(HostIp::All),
        Some(other) => bail!(
            "label {} must be \"true\" or \"false\", not {other:?}",
            labels::PUBLIC
        ),
    }
}

/// Parse the `ports` list. Every binding listens on `host_ip`.
pub(super) fn parse_ports(value: &Value, host_ip: HostIp) -> Result<Vec<PortBinding>> {
    let Value::Array(entries) = value else {
        bail!("ports must be a list");
    };
    let mut bindings = Vec::with_capacity(entries.len());
    for entry in entries {
        let parsed = match entry {
            Value::String(text) => parse_short(text, host_ip),
            Value::Number(n) => parse_short(&n.to_string(), host_ip),
            Value::Object(fields) => parse_long(fields, host_ip),
            _ => bail!("ports entries must be strings, numbers or mappings"),
        };
        match parsed {
            Ok(binding) => bindings.push(binding),
            Err(e) => return Err(e),
        }
    }
    Ok(bindings)
}

fn parse_port(text: &str, entry: &str) -> Result<u16> {
    match text.parse::<u16>() {
        Ok(port) if port != 0 => Ok(port),
        _ => bail!("port {entry:?}: {text:?} is not a port number (ranges are not supported)"),
    }
}

fn parse_protocol(text: &str, entry: &str) -> Result<Protocol> {
    match text {
        "tcp" => Ok(Protocol::Tcp),
        "udp" => Ok(Protocol::Udp),
        other => bail!("port {entry:?}: unsupported protocol {other:?}"),
    }
}

fn parse_short(text: &str, host_ip: HostIp) -> Result<PortBinding> {
    let (spec, protocol) = match text.rsplit_once('/') {
        Some((spec, protocol)) => match parse_protocol(protocol, text) {
            Ok(protocol) => (spec, protocol),
            Err(e) => return Err(e),
        },
        None => (text, Protocol::Tcp),
    };
    let (host, container) = match spec.split(':').collect::<Vec<_>>().as_slice() {
        [container] => (None, *container),
        [host, container] => (Some(*host), *container),
        _ => bail!("port {text:?}: {HOST_IP_REFUSAL}"),
    };
    let host_port = match host {
        Some(host) => match parse_port(host, text) {
            Ok(port) => Some(port),
            Err(e) => return Err(e),
        },
        None => None,
    };
    let container_port = match parse_port(container, text) {
        Ok(port) => port,
        Err(e) => return Err(e),
    };
    Ok(PortBinding {
        host_ip,
        host_port,
        container_port,
        protocol,
    })
}

fn port_field(fields: &super::Mapping, key: &str) -> Result<Option<u16>> {
    let text = match fields.get(key) {
        None | Some(Value::Null) => return Ok(None),
        Some(Value::String(text)) => text.clone(),
        Some(Value::Number(n)) => n.to_string(),
        Some(_) => bail!("port {key} must be a number"),
    };
    match parse_port(&text, &text) {
        Ok(port) => Ok(Some(port)),
        Err(e) => Err(e),
    }
}

fn parse_long(fields: &super::Mapping, host_ip: HostIp) -> Result<PortBinding> {
    if fields.contains_key("host_ip") || fields.contains_key("mode") {
        bail!("port {}: {HOST_IP_REFUSAL}", Value::Object(fields.clone()));
    }
    if let Err(e) = reject_unknown(fields, &["target", "published", "protocol"], "port") {
        return Err(e);
    }
    let container_port = match port_field(fields, "target") {
        Ok(Some(port)) => port,
        Ok(None) => bail!("port {} has no target", Value::Object(fields.clone())),
        Err(e) => return Err(e),
    };
    let host_port = match port_field(fields, "published") {
        Ok(port) => port,
        Err(e) => return Err(e),
    };
    let protocol = match fields.get("protocol") {
        None => Protocol::Tcp,
        Some(Value::String(text)) => match parse_protocol(text, text) {
            Ok(protocol) => protocol,
            Err(e) => return Err(e),
        },
        Some(_) => bail!("port protocol must be a string"),
    };
    Ok(PortBinding {
        host_ip,
        host_port,
        container_port,
        protocol,
    })
}

#[cfg(test)]
#[path = "ports_tests.rs"]
mod tests;
