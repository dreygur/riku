//! The `[routing]` section: how an app without a domain of its own gets a
//! URL.
//!
//! ```toml
//! [routing]
//! fallback_domain = "{app}.{ip}.example"
//! server_ip = "203.0.113.10"
//! shared_port = 8080
//! port_range = "8100-8199"
//! ```
//!
//! Security: the file is operator owned. The fallback pattern may hold only
//! hostname characters and the `{app}` and `{ip}` placeholders, and every
//! expansion is checked again as a [`Domain`] before it reaches a router
//! config, so an app name cannot smuggle config syntax through it.

use std::net::{IpAddr, Ipv4Addr};
use std::path::Path;

use anyhow::{anyhow, Result};
use riku_util::domain::Domain;

use super::engine::text_at;

const APP_PLACEHOLDER: &str = "{app}";
const IP_PLACEHOLDER: &str = "{ip}";
const DEFAULT_SHARED_PORT: u16 = 8080;
const DEFAULT_RANGE: PortRange = PortRange {
    first: 8100,
    last: 8199,
};
const FIRST_UNPRIVILEGED_PORT: u16 = 1024;
const CADDY_ADMIN_PORT: u16 = 2019;
/// The section text lives in a template so no naming service appears in the
/// code; riku only fills in the server IP line.
const SECTION_TEMPLATE: &str = include_str!("../../../../templates/settings_routing.toml.txt");
const SERVER_IP_MARKER: &str = "SERVER_IP_LINE";

/// A fallback domain pattern such as `{app}.{ip}.example`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FallbackPattern(String);

/// An inclusive range of ports riku may pick from for port mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PortRange {
    pub first: u16,
    pub last: u16,
}

/// Everything in `[routing]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutingSettings {
    /// `None` turns the fallback domain off.
    pub fallback_domain: Option<FallbackPattern>,
    /// The public IPv4 address `{ip}` expands to. `None` when unknown.
    pub server_ip: Option<Ipv4Addr>,
    /// The port path mode apps share.
    pub shared_port: u16,
    pub port_range: PortRange,
    /// False when the file has no `[routing]` section, which keeps a server
    /// that has not run `riku doctor --fix` routing exactly as before.
    pub present: bool,
}

impl Default for RoutingSettings {
    fn default() -> Self {
        RoutingSettings {
            fallback_domain: None,
            server_ip: None,
            shared_port: DEFAULT_SHARED_PORT,
            port_range: DEFAULT_RANGE,
            present: false,
        }
    }
}

impl TryFrom<&str> for FallbackPattern {
    type Error = String;

    fn try_from(raw: &str) -> Result<Self, String> {
        if !raw.contains(APP_PLACEHOLDER) {
            return Err(format!("must contain {APP_PLACEHOLDER}"));
        }
        let rest = raw.replace(APP_PLACEHOLDER, "").replace(IP_PLACEHOLDER, "");
        let hostname_chars = rest
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '.');
        if !hostname_chars {
            return Err(format!(
                "may hold only lowercase letters, digits, '-', '.', {APP_PLACEHOLDER} and {IP_PLACEHOLDER}"
            ));
        }
        Ok(FallbackPattern(raw.to_string()))
    }
}

impl FallbackPattern {
    /// Whether the pattern needs a server IP to expand.
    pub fn needs_ip(&self) -> bool {
        self.0.contains(IP_PLACEHOLDER)
    }

    /// The domain for `app`, or the reason there is none, for the push output.
    pub fn expand(&self, app: &str, ip: Ipv4Addr) -> Result<Domain, String> {
        if app.contains('.') {
            return Err(format!(
                "the app name '{app}' has a dot, so it would not be one label of the fallback name"
            ));
        }
        let host = self
            .0
            .replace(APP_PLACEHOLDER, app)
            .replace(IP_PLACEHOLDER, &ip.to_string());
        match Domain::try_from(host.as_str()) {
            Ok(domain) => Ok(domain),
            Err(_) => Err(format!(
                "the app name '{app}' does not make a valid hostname from the fallback pattern"
            )),
        }
    }
}

impl TryFrom<&str> for PortRange {
    type Error = String;

    fn try_from(raw: &str) -> Result<Self, String> {
        let bounds = raw.split_once('-').and_then(|(a, b)| {
            a.trim()
                .parse::<u16>()
                .ok()
                .zip(b.trim().parse::<u16>().ok())
        });
        match bounds {
            Some((first, last)) if first <= last && first >= FIRST_UNPRIVILEGED_PORT => {
                Ok(PortRange { first, last })
            }
            _ => Err(format!(
                "must be \"FIRST-LAST\" with {FIRST_UNPRIVILEGED_PORT} <= FIRST <= LAST <= 65535"
            )),
        }
    }
}

fn refused(file: &Path, key: &str, why: String) -> anyhow::Error {
    anyhow!("{}: routing.{key} {why}", file.display())
}

fn server_ip(value: &toml::Value, file: &Path) -> Result<Ipv4Addr> {
    let parsed = value.as_str().map(|text| text.trim().parse::<IpAddr>());
    match parsed {
        Some(Ok(IpAddr::V4(ip))) => Ok(ip),
        _ => Err(refused(
            file,
            "server_ip",
            "must be an IPv4 address".to_string(),
        )),
    }
}

fn port(value: &toml::Value, file: &Path) -> Result<u16> {
    match value.as_integer().map(u16::try_from) {
        Some(Ok(port)) if port >= FIRST_UNPRIVILEGED_PORT && port != CADDY_ADMIN_PORT => Ok(port),
        _ => Err(refused(
            file,
            "shared_port",
            format!("must be a whole number from {FIRST_UNPRIVILEGED_PORT} to 65535, not the Caddy admin port {CADDY_ADMIN_PORT}"),
        )),
    }
}

#[allow(clippy::question_mark)]
fn text_value<T>(
    key: &str,
    value: &toml::Value,
    file: &Path,
    convert: impl Fn(&str) -> Result<T, String>,
) -> Result<T> {
    let text = match text_at("routing", key, value, file) {
        Ok(text) => text,
        Err(e) => return Err(e),
    };
    convert(&text).map_err(|why| refused(file, key, why))
}

/// Parse the `[routing]` table.
#[allow(clippy::question_mark)]
pub(super) fn routing_settings(table: &toml::Table, file: &Path) -> Result<RoutingSettings> {
    let mut routing = RoutingSettings {
        present: true,
        ..RoutingSettings::default()
    };
    for (key, value) in table {
        let outcome = match key.as_str() {
            "fallback_domain" => text_value(key, value, file, |t| FallbackPattern::try_from(t))
                .map(|pattern| routing.fallback_domain = Some(pattern)),
            "server_ip" => server_ip(value, file).map(|ip| routing.server_ip = Some(ip)),
            "shared_port" => port(value, file).map(|port| routing.shared_port = port),
            "port_range" => text_value(key, value, file, |t| PortRange::try_from(t))
                .map(|range| routing.port_range = range),
            _ => Err(anyhow!("{}: unknown key routing.{key}", file.display())),
        };
        if let Err(e) = outcome {
            return Err(e);
        }
    }
    Ok(routing)
}

/// The `[routing]` text `riku init` and `riku doctor --fix` append. The
/// `server_ip` line is left out when no public IPv4 address is known, which
/// turns the fallback domain off until the operator adds it.
pub fn routing_section(ip: Option<Ipv4Addr>) -> String {
    let ip_line = match ip {
        Some(ip) => format!("server_ip = \"{ip}\""),
        None => "# server_ip = \"203.0.113.10\"  # the public IPv4 address; the fallback is off until set".to_string(),
    };
    SECTION_TEMPLATE.replace(SERVER_IP_MARKER, &ip_line)
}

#[cfg(test)]
#[path = "routing_tests.rs"]
mod tests;
