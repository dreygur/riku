//! The env a router sees: the app's own env with its fallback domain in
//! `RIKU_DOMAINS` and its resolved port mode port in `RIKU_ROUTING_PORT`.
//!
//! Every route builder, the plugin route plan and certificate renewal read
//! domains from `RIKU_DOMAINS`, so laying the fallback domain over a copy of
//! the env reaches all of them without each learning about the fallback.
//! `RIKU_HTTPS_ONLY` is dropped from that copy, because https only applies to
//! the app's own domains: a fallback certificate that cannot be issued must
//! never take the app offline. The app's ENV file is never changed.

use std::collections::HashMap;

use anyhow::Result;

use crate::config::router::Router;
use crate::config::{settings, RikuPaths};
use crate::router::app_routing::{resolve, AppRouting};
use crate::router::routing_state::{with_routing_lock, RoutingState};
use crate::util::listening::listening_ports;

const DOMAINS_KEY: &str = "RIKU_DOMAINS";
const HTTPS_ONLY_KEY: &str = "RIKU_HTTPS_ONLY";
const PORT_KEY: &str = "RIKU_ROUTING_PORT";

/// Whether resolving records the app's entry in the routing state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Record {
    /// A deploy or a router apply: picks and keeps ports, under the lock.
    Save,
    /// A check or a renewal: reads the state, changes nothing.
    Peek,
}

#[allow(clippy::question_mark)]
fn resolved(
    app: &str,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
    active: &Router,
    record: Record,
) -> Result<AppRouting> {
    let loaded = match settings::load(paths) {
        Ok(loaded) => loaded,
        Err(e) => return Err(e),
    };
    let plugin = matches!(active, Router::Plugin(_));
    let listening = listening_ports();
    let run =
        |state: &mut RoutingState| resolve(app, env, &loaded.routing, state, &listening, plugin);
    match record {
        Record::Save => with_routing_lock(paths, run),
        Record::Peek => match RoutingState::load(paths) {
            Ok(mut state) => run(&mut state),
            Err(e) => Err(e),
        },
    }
}

/// `app`'s routing and the env its router should see.
#[allow(clippy::question_mark)]
pub fn routing_env(
    app: &str,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
    active: &Router,
    record: Record,
) -> Result<(HashMap<String, String>, AppRouting)> {
    let routing = match resolved(app, env, paths, active, record) {
        Ok(routing) => routing,
        Err(e) => return Err(e),
    };
    let mut routed = env.clone();
    match routing.port {
        Some(port) => routed.insert(PORT_KEY.to_string(), port.to_string()),
        None => routed.remove(PORT_KEY),
    };
    if routing.fallback {
        routed.insert(DOMAINS_KEY.to_string(), routing.domains.join(","));
        routed.remove(HTTPS_ONLY_KEY);
    }
    Ok((routed, routing))
}

#[cfg(test)]
#[path = "routing_env_tests.rs"]
mod tests;
