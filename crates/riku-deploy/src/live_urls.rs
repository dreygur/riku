//! The lines a push ends with: where the app is live, or why it has no URL
//! and how to give it one.

use std::net::{IpAddr, Ipv4Addr};

use serde::Serialize;

use crate::config::router::Router;
use crate::router::app_routing::{AppRouting, RoutingMode};
pub(crate) use crate::router::routing_state::TlsState;
use crate::util::outbound_ip::public_v4;

/// nginx's "any host" name, which is not a URL.
const CATCH_ALL_NAME: &str = "_";

/// Who can open a live URL.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UrlLabel {
    /// One of the app's domains, its own or the fallback domain.
    Domain,
    /// A port or path URL on an address the internet cannot reach; the push
    /// output marks it `(LAN)`.
    Lan,
    /// A port or path URL on a public address.
    Public,
}

/// A URL the app is live at, as the push output lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LiveUrl {
    pub url: String,
    pub label: UrlLabel,
}

/// Whether the router serves the app's own domains over HTTPS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DomainTls {
    /// Plain HTTP only: nginx or a router plugin with no certificate for
    /// the app.
    PlainHttp,
    /// HTTPS on every name: the app is `https_only`, or nginx or Caddy
    /// serves the certificate its owner supplied.
    Served,
    /// Caddy's automatic HTTPS, which gets a public certificate for every
    /// hostname but not for a bare IP or a wildcard.
    CaddyAutomatic,
}

/// How `router` serves the app's own domains. Caddy writes each domain as a
/// bare site address next to its `http://` one, so it serves HTTPS on its
/// own; nginx serves 443 only with a certificate pair on the route; a router
/// plugin gets no certificate at all.
pub(crate) fn domain_tls(router: &Router, https_only: bool, own_cert: bool) -> DomainTls {
    match router {
        _ if https_only => DomainTls::Served,
        Router::Nginx | Router::Caddy if own_cert => DomainTls::Served,
        Router::Caddy => DomainTls::CaddyAutomatic,
        Router::Nginx | Router::Plugin(_) => DomainTls::PlainHttp,
    }
}

impl DomainTls {
    fn scheme(self, domain: &str) -> &'static str {
        let public_name = !domain.starts_with('*') && domain.parse::<IpAddr>().is_err();
        match self {
            DomainTls::Served => "https",
            DomainTls::CaddyAutomatic if public_name => "https",
            DomainTls::CaddyAutomatic | DomainTls::PlainHttp => "http",
        }
    }
}

/// One line of the push output and its colour. `url` is set on a "Live at"
/// line whose host is known.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Line {
    pub text: String,
    pub colour: &'static str,
    pub url: Option<LiveUrl>,
}

fn line(text: String, colour: &'static str) -> Line {
    Line {
        text,
        colour,
        url: None,
    }
}

/// The green "Live at" line for `url`, with `suffix` after it in the text.
fn live_line(url: String, label: UrlLabel, suffix: &str) -> Line {
    Line {
        text: format!("Live at {url}{suffix}"),
        colour: "green",
        url: Some(LiveUrl { url, label }),
    }
}

fn domain_line(scheme: &str, domain: &str) -> Line {
    live_line(format!("{scheme}://{domain}"), UrlLabel::Domain, "")
}

fn fallback_lines(domain: &str, tls: Option<TlsState>, private_ip: Option<Ipv4Addr>) -> Vec<Line> {
    if let Some(ip) = private_ip {
        return vec![
            domain_line("http", domain),
            line(
                format!("HTTP only: {ip} is a private address, so no certificate can be issued"),
                "",
            ),
        ];
    }
    match tls {
        Some(TlsState::Issued) => vec![domain_line("https", domain)],
        Some(TlsState::ByCaddy) => vec![
            domain_line("https", domain),
            line(
                format!("Caddy is issuing the certificate; http://{domain} works meanwhile"),
                "",
            ),
        ],
        _ => vec![
            domain_line("http", domain),
            line(
                "HTTPS is not available for this name yet; the app answers on plain HTTP"
                    .to_string(),
                "yellow",
            ),
        ],
    }
}

fn no_domain_lines(app: &str, routing: &AppRouting) -> Vec<Line> {
    let mut lines: Vec<Line> = routing
        .fallback_skipped
        .iter()
        .map(|why| line(format!("No fallback domain: {why}"), "yellow"))
        .collect();
    lines.push(line(
        format!(
            "No domain set: add domains under [routing] in riku.toml, or run rk settings set {app} routing.domains=<domain>"
        ),
        "",
    ));
    lines
}

/// The push output for `app` routed as `routing`. `tls` is the fallback
/// certificate outcome; `own_tls` is how the router serves the app's own
/// domains.
pub(crate) fn live_lines(
    app: &str,
    routing: &AppRouting,
    tls: Option<TlsState>,
    own_tls: DomainTls,
) -> Vec<Line> {
    let mode_has_own_lines = routing.port.is_some() || routing.mode == RoutingMode::Path;
    if routing.domains.is_empty() && mode_has_own_lines {
        return Vec::new();
    }
    if routing.domains.is_empty() {
        return no_domain_lines(app, routing);
    }
    if routing.fallback {
        return fallback_lines(&routing.domains[0], tls, routing.fallback_private_ip);
    }
    routing
        .domains
        .iter()
        .filter(|domain| domain.as_str() != CATCH_ALL_NAME)
        .map(|domain| domain_line(own_tls.scheme(domain), domain))
        .collect()
}

/// The host for a port or path URL: the `configured` server_ip, else the
/// IPv4 address this host was `detected` to use, which may be private.
pub(crate) fn url_host(configured: Option<Ipv4Addr>, detected: Option<IpAddr>) -> Option<Ipv4Addr> {
    match (configured, detected) {
        (Some(ip), _) => Some(ip),
        (None, Some(IpAddr::V4(ip))) => Some(ip),
        _ => None,
    }
}

/// The "Live at" line for a port or path URL on `server_ip` (`path` starts
/// with `:`). An address the internet cannot reach gets ` (LAN)` after the
/// URL, so a pusher elsewhere knows the link only works on the server's
/// network. An unknown address prints a placeholder and carries no URL.
fn host_line(server_ip: Option<Ipv4Addr>, path: &str) -> Line {
    match server_ip {
        Some(ip) if public_v4(IpAddr::V4(ip)).is_some() => {
            live_line(format!("http://{ip}{path}"), UrlLabel::Public, "")
        }
        Some(ip) => live_line(format!("http://{ip}{path}"), UrlLabel::Lan, " (LAN)"),
        None => line(
            format!("Live at http://<this server's address>{path}"),
            "green",
        ),
    }
}

/// The push output for an app answering on `port`. `server_ip` is the
/// address from settings.toml, when known.
pub(crate) fn port_lines(port: u16, server_ip: Option<Ipv4Addr>) -> Vec<Line> {
    vec![
        host_line(server_ip, &format!(":{port}")),
        line(
            format!("Open port {port} in the server's firewall if it is closed"),
            "",
        ),
        line(
            "A bare IP address gets HTTP only, no certificate".to_string(),
            "",
        ),
    ]
}

/// The push output for a path mode app on the shared port.
pub(crate) fn path_lines(app: &str, shared_port: u16, server_ip: Option<Ipv4Addr>) -> Vec<Line> {
    vec![
        host_line(server_ip, &format!(":{shared_port}/{app}/")),
        line(
            format!("The app must handle the base path /{app}: requests reach it without the prefix, which is sent as X-Forwarded-Prefix"),
            "yellow",
        ),
    ]
}

#[cfg(test)]
#[path = "live_urls_tests.rs"]
mod tests;
