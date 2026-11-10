//! Finding the server's public IPv4 address for the fallback domain, and
//! adding the `[routing]` section to settings.toml.
//!
//! Detection "connects" a UDP socket to a documentation address and reads
//! the local address the kernel chose. No packet is sent. Only a public IPv4
//! address is used: a private one would give a fallback URL that does not
//! work from the internet.

use std::net::{IpAddr, Ipv4Addr, UdpSocket};

use anyhow::Result;

use crate::config::{settings, RikuPaths};

/// TEST-NET-1 (RFC 5737). Only its route matters; nothing is sent to it.
const PROBE_ADDRESS: &str = "192.0.2.1:9";
const UNSPECIFIED_LOCAL: &str = "0.0.0.0:0";

/// The address this host would use for outbound traffic, if any.
pub fn detect_server_ip() -> Option<IpAddr> {
    let socket = match UdpSocket::bind(UNSPECIFIED_LOCAL) {
        Ok(socket) => socket,
        Err(_) => return None,
    };
    if socket.connect(PROBE_ADDRESS).is_err() {
        return None;
    }
    socket.local_addr().ok().map(|addr| addr.ip())
}

fn shared_address_space(ip: Ipv4Addr) -> bool {
    let [first, second, ..] = ip.octets();
    first == 100 && (64..128).contains(&second)
}

/// `ip` when it is an IPv4 address reachable from the internet.
pub fn public_v4(ip: IpAddr) -> Option<Ipv4Addr> {
    let IpAddr::V4(v4) = ip else {
        return None;
    };
    let local = v4.is_private()
        || v4.is_loopback()
        || v4.is_link_local()
        || v4.is_unspecified()
        || v4.is_broadcast()
        || shared_address_space(v4);
    if local {
        return None;
    }
    Some(v4)
}

/// What init prints about the fallback domain.
pub fn routing_note(detected: Option<IpAddr>, public: Option<Ipv4Addr>) -> String {
    match (public, detected) {
        (Some(ip), _) => format!("      ✓ fallback domain on, server_ip {ip}"),
        (None, Some(ip)) => format!(
            "      ! this host's address {ip} is not public; the fallback domain is off.\n        Add server_ip = \"<public IPv4>\" under [routing] in settings.toml to turn it on."
        ),
        (None, None) => "      ! no server address found; the fallback domain is off.\n        Add server_ip = \"<public IPv4>\" under [routing] in settings.toml to turn it on.".to_string(),
    }
}

/// Add `[routing]` to `paths`' settings.toml when it has none, filling in
/// the public IPv4 address when one is found. Returns the note to print, or
/// `None` when the section was already there.
#[allow(clippy::question_mark)]
pub fn ensure_routing_section(paths: &RikuPaths) -> Result<Option<String>> {
    let detected = detect_server_ip();
    let public = detected.and_then(public_v4);
    match settings::add_routing_section(paths, public) {
        Ok(true) => Ok(Some(routing_note(detected, public))),
        Ok(false) => Ok(None),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
#[path = "server_ip_tests.rs"]
mod tests;
