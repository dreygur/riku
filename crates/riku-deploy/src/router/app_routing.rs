//! Turning an app's riku settings and the server's `[routing]` section into
//! how the app is reached: its domains (its own or the fallback), the port
//! it answers on and its mode.
//!
//! The rules, in order:
//! - No `[routing]` section: the app routes exactly as before this existed
//!   (`legacy`), and `routing.mode` or `routing.port` are refused.
//! - `mode = "path"` takes no domains and no port.
//! - An app with no domain gets the fallback domain, unless it asked for
//!   port mode; with no fallback it falls into port mode.
//! - A port the app sets is checked; a port mode app without one gets one
//!   from the range, kept in the routing state while it stays in port mode.
//! - A router plugin only does domain mode.

use std::collections::{BTreeSet, HashMap};

use anyhow::{anyhow, Result};

use crate::config::settings::RoutingSettings;
use crate::router::port_rules::{check_port, pick_port};
use crate::router::routing_state::{Entry, RoutingState};

pub use crate::router::routing_state::RoutingMode;

const DOMAINS_KEY: &str = "RIKU_DOMAINS";
const MODE_KEY: &str = "RIKU_ROUTING_MODE";
const PORT_KEY: &str = "RIKU_ROUTING_PORT";
/// uWSGI and static sites have no port mode listener, so a domainless one
/// stays in domain mode and routes as it did before port mode existed.
const WSGI_KEY: &str = "RIKU_NGINX_WSGI";
const STATIC_KEY: &str = "RIKU_NGINX_STATIC";

/// How one app is reached.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppRouting {
    /// The app's own domains as written (nginx's `_` included), or the one
    /// fallback domain. Checked again where a route is built.
    pub domains: Vec<String>,
    /// True when `domains` is the fallback domain.
    pub fallback: bool,
    /// The port the app answers on for any host name, besides its domains.
    pub port: Option<u16>,
    pub mode: RoutingMode,
    /// True on a server with no `[routing]` section.
    pub legacy: bool,
    /// Why an app without a domain got no fallback domain, for the push.
    pub fallback_skipped: Option<String>,
}

fn env_domains(env: &HashMap<String, String>) -> Vec<String> {
    env.get(DOMAINS_KEY)
        .map(|raw| {
            raw.split(|c: char| c == ',' || c.is_whitespace())
                .filter(|part| !part.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn env_mode(env: &HashMap<String, String>) -> Result<Option<RoutingMode>> {
    match env.get(MODE_KEY).map(String::as_str) {
        None => Ok(None),
        Some("domain") => Ok(Some(RoutingMode::Domain)),
        Some("port") => Ok(Some(RoutingMode::Port)),
        Some("path") => Ok(Some(RoutingMode::Path)),
        Some(_) => Err(anyhow!(
            "routing.mode must be \"domain\", \"port\" or \"path\""
        )),
    }
}

fn env_port(env: &HashMap<String, String>) -> Result<Option<u16>> {
    match env.get(PORT_KEY) {
        None => Ok(None),
        Some(raw) => match raw.parse::<u16>() {
            Ok(port) if port > 0 => Ok(Some(port)),
            _ => Err(anyhow!(
                "routing.port must be a port number from 1 to 65535"
            )),
        },
    }
}

fn routed(domains: Vec<String>, mode: RoutingMode) -> AppRouting {
    AppRouting {
        domains,
        fallback: false,
        port: None,
        mode,
        legacy: false,
        fallback_skipped: None,
    }
}

/// The fallback domain for `app`, or why there is none.
fn fallback_domain(app: &str, settings: &RoutingSettings) -> Result<String, Option<String>> {
    let pattern = match &settings.fallback_domain {
        Some(pattern) => pattern,
        None => return Err(None),
    };
    let ip = match settings.server_ip {
        Some(ip) => ip,
        None if pattern.needs_ip() => {
            return Err(Some(
                "settings.toml has no routing.server_ip, so the fallback domain is off".to_string(),
            ))
        }
        None => std::net::Ipv4Addr::UNSPECIFIED,
    };
    match pattern.expand(app, ip) {
        Ok(domain) => Ok(domain.as_str().to_string()),
        Err(why) => Err(Some(why)),
    }
}

#[allow(clippy::question_mark)]
fn port_for(
    app: &str,
    set: Option<u16>,
    mode: RoutingMode,
    state: &RoutingState,
    settings: &RoutingSettings,
    listening: &BTreeSet<u16>,
) -> Result<(Option<u16>, bool)> {
    if let Some(port) = set {
        return check_port(app, port, state, settings, listening).map(|()| (Some(port), false));
    }
    if mode != RoutingMode::Port {
        return Ok((None, false));
    }
    let kept = state
        .entry(app)
        .filter(|entry| entry.picked)
        .and_then(|entry| entry.port)
        .filter(|port| check_port(app, *port, state, settings, listening).is_ok());
    if let Some(port) = kept {
        return Ok((Some(port), true));
    }
    pick_port(app, state, settings, listening).map(|port| (Some(port), true))
}

/// Resolve `app`'s routing and record it in `state`. `listening` is what the
/// host already listens on; `plugin_router` is true under a router plugin.
#[allow(clippy::question_mark)]
pub fn resolve(
    app: &str,
    env: &HashMap<String, String>,
    settings: &RoutingSettings,
    state: &mut RoutingState,
    listening: &BTreeSet<u16>,
    plugin_router: bool,
) -> Result<AppRouting> {
    let domains = env_domains(env);
    let (mode_set, port_set) =
        match env_mode(env).and_then(|mode| env_port(env).map(|port| (mode, port))) {
            Ok(pair) => pair,
            Err(e) => return Err(e),
        };
    if !settings.present {
        if mode_set.is_some() || port_set.is_some() {
            return Err(anyhow!(
                "routing.mode and routing.port need a [routing] section in settings.toml; run riku doctor --fix on the server"
            ));
        }
        return Ok(AppRouting {
            legacy: true,
            ..routed(domains, RoutingMode::Domain)
        });
    }
    if plugin_router
        && (port_set.is_some() || matches!(mode_set, Some(RoutingMode::Port | RoutingMode::Path)))
    {
        return Err(anyhow!("the router plugin supports domain mode only"));
    }
    let kept_cert = state.entry(app).and_then(|entry| entry.cert_for.clone());
    if mode_set == Some(RoutingMode::Path) {
        if !domains.is_empty() || port_set.is_some() {
            return Err(anyhow!("path mode takes no domains or port"));
        }
        state.set_entry(
            app,
            Entry {
                mode: RoutingMode::Path,
                port: None,
                picked: false,
                tls: None,
                cert_for: kept_cert.clone(),
            },
        );
        return Ok(routed(Vec::new(), RoutingMode::Path));
    }

    let mut routing = routed(domains, RoutingMode::Domain);
    if routing.domains.is_empty() && mode_set != Some(RoutingMode::Port) {
        match fallback_domain(app, settings) {
            Ok(domain) => {
                routing.domains = vec![domain];
                routing.fallback = true;
            }
            Err(why) => routing.fallback_skipped = why,
        }
    }
    let serves_a_port = !env.contains_key(WSGI_KEY) && !env.contains_key(STATIC_KEY);
    let implicit_port = routing.domains.is_empty() && !plugin_router && serves_a_port;
    routing.mode = match mode_set {
        Some(mode) => mode,
        None if implicit_port => RoutingMode::Port,
        None => RoutingMode::Domain,
    };

    let (port, picked) = match port_for(app, port_set, routing.mode, state, settings, listening) {
        Ok(found) => found,
        Err(e) => return Err(e),
    };
    routing.port = port;
    let tls = state
        .entry(app)
        .and_then(|entry| entry.tls)
        .filter(|_| routing.fallback);
    state.set_entry(
        app,
        Entry {
            mode: routing.mode,
            port,
            picked,
            tls,
            cert_for: kept_cert,
        },
    );
    Ok(routing)
}

#[cfg(test)]
#[path = "app_routing_tests.rs"]
mod tests;
