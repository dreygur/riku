//! The lines a push ends with: where the app is live, or why it has no URL
//! and how to give it one.

use std::net::{IpAddr, Ipv4Addr};

use crate::router::app_routing::{AppRouting, RoutingMode};
pub(crate) use crate::router::routing_state::TlsState;
use crate::util::outbound_ip::public_v4;

/// nginx's "any host" name, which is not a URL.
const CATCH_ALL_NAME: &str = "_";

/// One line of the push output and its colour.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Line {
    pub text: String,
    pub colour: &'static str,
}

fn line(text: String, colour: &'static str) -> Line {
    Line { text, colour }
}

fn fallback_lines(domain: &str, tls: Option<TlsState>) -> Vec<Line> {
    match tls {
        Some(TlsState::Issued) => vec![line(format!("Live at https://{domain}"), "green")],
        Some(TlsState::ByCaddy) => vec![
            line(format!("Live at http://{domain}"), "green"),
            line(
                format!(
                    "Caddy is getting an HTTPS certificate; https://{domain} works once it has one"
                ),
                "",
            ),
        ],
        _ => vec![
            line(format!("Live at http://{domain}"), "green"),
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
    if routing.legacy {
        lines.push(line(
            "Run sudo riku init on the server to give apps without a domain a URL".to_string(),
            "",
        ));
    }
    lines
}

/// The push output for `app` routed as `routing`. `tls` is the fallback
/// certificate outcome; `https_only` is the app's own setting.
pub(crate) fn live_lines(
    app: &str,
    routing: &AppRouting,
    tls: Option<TlsState>,
    https_only: bool,
) -> Vec<Line> {
    let mode_has_own_lines = routing.port.is_some() || routing.mode == RoutingMode::Path;
    if routing.domains.is_empty() && mode_has_own_lines {
        return Vec::new();
    }
    if routing.domains.is_empty() {
        return no_domain_lines(app, routing);
    }
    if routing.fallback {
        return fallback_lines(&routing.domains[0], tls);
    }
    let scheme = if https_only { "https" } else { "http" };
    routing
        .domains
        .iter()
        .filter(|domain| domain.as_str() != CATCH_ALL_NAME)
        .map(|domain| line(format!("Live at {scheme}://{domain}"), "green"))
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

/// The host of a port or path URL, and the label after the URL: ` (LAN)`
/// for an address the internet cannot reach, so a pusher elsewhere knows
/// the link only works on the server's network.
fn host_and_label(server_ip: Option<Ipv4Addr>) -> (String, &'static str) {
    match server_ip {
        Some(ip) if public_v4(IpAddr::V4(ip)).is_some() => (ip.to_string(), ""),
        Some(ip) => (ip.to_string(), " (LAN)"),
        None => ("<this server's address>".to_string(), ""),
    }
}

/// The push output for an app answering on `port`. `server_ip` is the
/// address from settings.toml, when known.
pub(crate) fn port_lines(port: u16, server_ip: Option<Ipv4Addr>) -> Vec<Line> {
    let (host, label) = host_and_label(server_ip);
    vec![
        line(format!("Live at http://{host}:{port}{label}"), "green"),
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
    let (host, label) = host_and_label(server_ip);
    vec![
        line(
            format!("Live at http://{host}:{shared_port}/{app}/{label}"),
            "green",
        ),
        line(
            format!("The app must handle the base path /{app}: requests reach it without the prefix, which is sent as X-Forwarded-Prefix"),
            "yellow",
        ),
    ]
}

#[cfg(test)]
#[path = "live_urls_tests.rs"]
mod tests;
