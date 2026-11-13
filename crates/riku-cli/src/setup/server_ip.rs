//! Finding the server's public IPv4 address for the fallback domain, and
//! adding the `[routing]` section to settings.toml.
//!
//! Detection is [`crate::util::outbound_ip::detect_outbound_ip`]. Only a
//! public IPv4 address is used: a private one would give a fallback URL that
//! does not work from the internet.

use std::net::{IpAddr, Ipv4Addr};

use anyhow::Result;

use crate::config::{settings, RikuPaths};
use crate::util::outbound_ip::detect_outbound_ip;
pub use crate::util::outbound_ip::public_v4;

/// Whether the fallback domain came out on when init wrote `[routing]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FallbackDomain {
    /// On, built from this public address.
    On(Ipv4Addr),
    /// Off, with why no public address was found.
    Off(String),
}

/// How to turn the fallback domain on by hand.
pub const SERVER_IP_HINT: &str =
    "Add server_ip = \"<public IPv4>\" under [routing] in settings.toml to turn it on.";

impl std::fmt::Display for FallbackDomain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FallbackDomain::On(ip) => write!(f, "fallback domain on, server_ip {ip}"),
            FallbackDomain::Off(why) => write!(f, "fallback domain off: {why}. {SERVER_IP_HINT}"),
        }
    }
}

/// The fallback domain outcome for a `detected` address and its `public`
/// form.
pub fn routing_note(detected: Option<IpAddr>, public: Option<Ipv4Addr>) -> FallbackDomain {
    match (public, detected) {
        (Some(ip), _) => FallbackDomain::On(ip),
        (None, Some(ip)) => FallbackDomain::Off(format!("{ip} is not a public address")),
        (None, None) => FallbackDomain::Off("no server address found".to_string()),
    }
}

/// Add `[routing]` to `paths`' settings.toml when it has none, filling in
/// the public IPv4 address when one is found. Returns the fallback domain
/// outcome, or `None` when the section was already there.
#[allow(clippy::question_mark)]
pub fn ensure_routing_section(paths: &RikuPaths) -> Result<Option<FallbackDomain>> {
    let detected = detect_outbound_ip();
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
