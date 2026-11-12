//! Router seam orchestrator (service layer).
//!
//! Resolves the active router and routes app network exposure to nginx or
//! Caddy through the router helper, or to a router *plugin* via the
//! [`crate::plugins::router`] seam. Callers (deploy, destroy) go through here
//! instead of reaching into nginx directly, so swapping the router is a config
//! change, not a code change. See `PLUGIN_PROTOCOL.md` §6.2.
//!
//! The router is a **host-level singleton**, read from `<riku root>/ROUTER`
//! (see [`Router::load`], default `nginx`): not a per-app setting. Per-app
//! `ENV` still shapes the *contents* of a router config (domains, port, TLS),
//! but never which router is active.
//!
//! Security: the request handed to a plugin is built only from a fixed set of
//! the app's own `ENV` keys, serialized as JSON, no shell interpolation, no
//! caller-controlled argv. The plugin runs under the shared plugin timeout.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{anyhow, Result};
use riku_container::engine::Engine;
use riku_route::Request;

use crate::config::router::Router;
use crate::config::RikuPaths;
use crate::util::backend::Backend;
use crate::util::domain::parse_domains;
use crate::util::flag::https_only;
use crate::util::route_plan::{HttpsPolicy, RoutePlan};

pub(crate) mod app_routing;
mod backends;
mod caddy;
mod check;
mod fallback;
mod fallback_tls;
mod helper_client;
mod nginx_site;
mod paths;
mod plugin;
mod port_rules;
mod reload;
mod remove;
mod renew;
mod route_build;
mod route_env;
pub(crate) mod routing_env;
pub(crate) mod routing_state;

pub use backends::{copy_socket, current_backends};
pub(crate) use backends::{parsed_worker, web_copies, worker_backend};
use caddy::apply_caddy_site;
pub use check::check;
use fallback::fallback_backends;
use fallback_tls::mark_caddy_tls;
pub use helper_client::send as send_to_helper;
use nginx_site::apply_nginx_site;
use paths::{previous_entry, publish_paths, route_by_path};
use plugin::configure_plugin;
pub use reload::ERROR_FILE as RELOAD_ERROR_FILE;
use reload::{record_outcome, reload_router_plugin};
pub use remove::remove;
pub use renew::renew;
pub use route_build::build_route;
use routing_env::{routing_env, Record};
use routing_state::{restore, RoutingMode};

/// Caddy reaches an app over loopback whatever `RIKU_BIND_ADDRESS` says.
/// nginx and router plugins keep the address each copy listens on, since a
/// plugin may run elsewhere and must not be told a made up address.
fn backends_for(router: &Router, backends: Vec<Backend>) -> Vec<Backend> {
    if !matches!(router, Router::Caddy) {
        return backends;
    }
    backends
        .into_iter()
        .map(|backend| match backend {
            Backend::Tcp { port, .. } => Backend::loopback(port),
            unix => unix,
        })
        .collect()
}

/// A container app (`engine` given) routes to its running routed copies; a
/// Procfile app to its web copies, or, a static site with none, to its
/// default backend.
#[allow(clippy::question_mark)]
fn routed_backends(
    active: &Router,
    app: &str,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
    engine: Option<&Engine>,
) -> Result<Vec<Backend>> {
    if engine.is_none() {
        let copies = match web_copies(app, paths) {
            Ok(copies) => copies,
            Err(e) => return Err(anyhow!("cannot route '{app}': {e}")),
        };
        if copies.is_empty() {
            tracing::debug!("'{app}' has no web copy, routing it to its default backend");
            return fallback_backends(active, app, env, paths);
        }
    }
    match current_backends(app, env, paths, engine) {
        Ok(backends) => Ok(backends_for(active, backends)),
        Err(e) => Err(anyhow!("cannot route '{app}': {e}")),
    }
}

/// Whether a Procfile app gets a route: it runs a web facing copy, or the
/// router serves its static files. A worker only app answers no HTTP.
pub(crate) fn has_route(
    app: &str,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
) -> Result<bool> {
    if app_routing::serves_static_site(env) {
        return Ok(true);
    }
    match web_copies(app, paths) {
        Ok(copies) => Ok(!copies.is_empty()),
        Err(e) => Err(anyhow!("cannot route '{app}': {e}")),
    }
}

/// Configure the active router so `app` is reachable, with a backend for
/// every web copy it has today, and return once the router confirmed the new
/// config. `engine` is `Some` for a container app. A Procfile app with no
/// route (see [`has_route`]) is taken off the router instead, which also
/// forgets its recorded port.
#[allow(clippy::question_mark)]
pub fn configure(
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
    engine: Option<&Engine>,
) -> Result<()> {
    if engine.is_none() {
        match has_route(app, env, paths) {
            Ok(true) => {}
            Ok(false) => return remove(app, paths),
            Err(e) => return Err(e),
        }
    }
    let active = match Router::load(paths) {
        Ok(router) => router,
        Err(e) => return Err(e),
    };
    let backends = match routed_backends(&active, app, env, paths, engine) {
        Ok(backends) => backends,
        Err(e) => return Err(e),
    };
    apply_to(&active, app, app_path, env, paths, &backends)
}

/// Install `app`'s router config for `backends` and wait for the active
/// router to confirm the reload. nginx and Caddy go through the router
/// helper.
#[allow(clippy::question_mark)]
pub fn apply(
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
    backends: &[Backend],
) -> Result<()> {
    let active = match Router::load(paths) {
        Ok(router) => router,
        Err(e) => return Err(e),
    };
    apply_to(&active, app, app_path, env, paths, backends)
}

#[allow(clippy::question_mark)]
fn apply_to(
    active: &Router,
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
    backends: &[Backend],
) -> Result<()> {
    let previous = previous_entry(app, paths);
    let was_path = previous.as_ref().map(|entry| entry.mode) == Some(RoutingMode::Path);
    let (env, routing) = match routing_env(app, env, paths, active, Record::Save) {
        Ok(routed) => routed,
        Err(e) => return Err(anyhow!("cannot route '{app}': {e}")),
    };
    let env = &env;
    if routing.mode == RoutingMode::Path {
        let outcome = route_by_path(active, app, env, paths, backends, was_path);
        if outcome.is_err() {
            restore(paths, app, previous);
        }
        record_outcome(paths, &outcome);
        return outcome;
    }
    let outcome = match active {
        Router::Nginx => {
            let send = |request: &Request| send_to_helper(paths, request);
            let outcome = apply_nginx_site(send, app, app_path, env, &routing, backends, paths);
            record_outcome(paths, &outcome);
            outcome
        }
        Router::Caddy => {
            let send = |request: &Request| send_to_helper(paths, request);
            let outcome = apply_caddy_site(send, app, env, backends, paths);
            if outcome.is_ok() && routing.fallback {
                mark_caddy_tls(app, paths);
            }
            record_outcome(paths, &outcome);
            outcome
        }
        Router::Plugin(name) => match configure_plugin(name, app, env, backends, paths) {
            Ok(()) => reload_router_plugin(name, paths),
            Err(e) => Err(e),
        },
    };
    if outcome.is_err() {
        restore(paths, app, previous);
        return outcome;
    }
    if was_path {
        return publish_paths(paths, active, (app, backends.to_vec()), true);
    }
    outcome
}

/// The routing plan for an app: its domains and HTTPS policy from ENV, and
/// the given `backends`.
///
/// A malformed `RIKU_DOMAINS` is an error, so nothing unvalidated reaches a
/// router.
fn route_plan(env: &HashMap<String, String>, backends: &[Backend]) -> Result<RoutePlan> {
    let raw = env
        .get("RIKU_DOMAINS")
        .map(String::as_str)
        .unwrap_or_default();
    let domains = match parse_domains(raw) {
        Ok(domains) => domains,
        Err(e) => return Err(anyhow!("invalid RIKU_DOMAINS: {e}")),
    };
    let https = if https_only(env) {
        HttpsPolicy::Only
    } else {
        HttpsPolicy::AlsoPlainHttp
    };
    Ok(RoutePlan {
        domains,
        https,
        backends: backends.to_vec(),
        open_port: None,
    })
}

#[cfg(test)]
#[path = "router_tests.rs"]
mod tests;
