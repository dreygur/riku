//! Which port an app in port mode may listen on.
//!
//! A port is refused when it is below 1024 (80 and 443 serve domains, and a
//! bare IP cannot get a certificate for 443), when it is the Caddy admin
//! port or the shared path port, when another app holds it in the routing
//! state, or when anything on the host already listens on it. The app's own
//! port stays allowed, since its router site listens there after the first
//! deploy.

use std::collections::BTreeSet;

use anyhow::{anyhow, Result};

use crate::config::settings::RoutingSettings;
use crate::router::routing_state::RoutingState;

const FIRST_UNPRIVILEGED_PORT: u16 = 1024;
const CADDY_ADMIN_PORT: u16 = 2019;

fn holder(state: &RoutingState, port: u16) -> Option<&str> {
    state
        .entries()
        .find(|(_, entry)| entry.port == Some(port))
        .map(|(app, _)| app)
}

/// Refuse `port` for `app`, naming why.
pub fn check_port(
    app: &str,
    port: u16,
    state: &RoutingState,
    settings: &RoutingSettings,
    listening: &BTreeSet<u16>,
) -> Result<()> {
    if port < FIRST_UNPRIVILEGED_PORT {
        return Err(anyhow!(
            "routing.port {port} is refused: use {FIRST_UNPRIVILEGED_PORT} or above (80 and 443 serve domains)"
        ));
    }
    if port == CADDY_ADMIN_PORT {
        return Err(anyhow!(
            "routing.port {port} is refused: it is the Caddy admin port"
        ));
    }
    if port == settings.shared_port {
        return Err(anyhow!(
            "routing.port {port} is refused: it is the shared port for path mode"
        ));
    }
    match holder(state, port) {
        Some(owner) if owner == app => return Ok(()),
        Some(owner) => {
            return Err(anyhow!(
                "routing.port {port} is refused: app '{owner}' already uses it"
            ))
        }
        None => {}
    }
    if listening.contains(&port) {
        return Err(anyhow!(
            "routing.port {port} is refused: something on this server is already listening on it"
        ));
    }
    Ok(())
}

/// The lowest port in the range `app` may take.
pub fn pick_port(
    app: &str,
    state: &RoutingState,
    settings: &RoutingSettings,
    listening: &BTreeSet<u16>,
) -> Result<u16> {
    let range = settings.port_range;
    (range.first..=range.last)
        .find(|port| check_port(app, *port, state, settings, listening).is_ok())
        .ok_or_else(|| {
            anyhow!(
                "no free port left in port_range {}-{}; set routing.port in riku.toml or widen the range in settings.toml",
                range.first,
                range.last
            )
        })
}

#[cfg(test)]
#[path = "port_rules_tests.rs"]
mod tests;
