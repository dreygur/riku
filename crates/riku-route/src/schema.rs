//! The route as it travels between the deploy side and the router helper.
//!
//! Every struct refuses unknown fields. Text fields are plain strings here
//! on purpose: a value of these types has been parsed, not checked, and only
//! [`crate::check_nginx_route`] or [`crate::check_caddy_route`] says it is safe
//! to render.

use std::fmt;
use std::net::IpAddr;

use riku_util::route_plan::HttpsPolicy;
use serde::{Deserialize, Serialize};

/// Everything the helper needs to route one app.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Route {
    /// Lowercase hostnames, at least one. nginx also takes `_`, its catch
    /// all name.
    pub domains: Vec<String>,
    pub https: HttpsPolicy,
    /// Every running copy of the web process, in copy order.
    pub backends: Vec<RouteBackend>,
    #[serde(default)]
    pub listen: Listen,
    /// Certificate and key for `https: only` on nginx. Caddy gets its own.
    pub tls: Option<Tls>,
    /// Location prefix of the n-th published static path.
    #[serde(default)]
    pub static_paths: Vec<String>,
    /// Which template shape the site takes.
    #[serde(default)]
    pub kind: SiteKind,
    #[serde(default)]
    pub allow_git_folders: bool,
    /// A plain HTTP port that answers any host name, for port mode. Left out
    /// of the JSON when unset, so a helper from before port mode still takes
    /// every route that does not use it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open_port: Option<u16>,
}

/// The shape of the site, in the order nginx's template choice tries them
/// (after `https: only`, which takes the HTTPS template whatever the kind).
/// One value, so contradictory combinations cannot be written.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SiteKind {
    /// A proxied app on the plain HTTP port 80.
    #[default]
    Plain,
    /// A proxied app on `listen.port`, served by a port mapping.
    Portmap,
    /// An app served by uWSGI over unix sockets, on `listen.port`.
    Wsgi,
    /// The published document root, with no upstream.
    Static,
}

/// One upstream. No form carries a path.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum RouteBackend {
    /// A loopback TCP port.
    Tcp { host: IpAddr, port: u16 },
    /// The unix socket of this copy number, `<socket root>/<app>@<n>.sock`.
    UnixCopy(u32),
    /// The app's single socket, `<socket root>/<app>.sock`, which an app
    /// with no web copy falls back to.
    UnixDefault,
}

/// Where the router listens for plain HTTP. The router listens on every
/// address, IPv4 and IPv6.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Listen {
    /// The plain HTTP port. HTTPS is always 443.
    pub port: u16,
}

impl Default for Listen {
    fn default() -> Self {
        Listen { port: 80 }
    }
}

/// PEM text for one site. The key never appears in a `Debug` print.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tls {
    /// The certificate chain, leaf first.
    pub cert_pem: String,
    /// The one private key matching the leaf.
    pub key_pem: String,
}

impl fmt::Debug for Tls {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Tls")
            .field("cert_pem", &self.cert_pem)
            .field("key_pem", &"<redacted>")
            .finish()
    }
}

#[cfg(test)]
#[path = "schema_tests.rs"]
mod tests;
