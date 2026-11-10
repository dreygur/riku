//! The lines a push ends with: where the app is live, or why it has no URL
//! and how to give it one.

use crate::router::app_routing::AppRouting;
pub(crate) use crate::router::routing_state::TlsState;

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
            "No domain set: add domains under [routing] in riku.toml, or run rk config set {app} RIKU_DOMAINS=<domain>"
        ),
        "",
    ));
    if routing.legacy {
        lines.push(line(
            "Run riku doctor --fix on the server to give apps without a domain a URL".to_string(),
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
        .map(|domain| line(format!("Live at {scheme}://{domain}"), "green"))
        .collect()
}

#[cfg(test)]
#[path = "live_urls_tests.rs"]
mod tests;
