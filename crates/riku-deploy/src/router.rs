//! Router seam orchestrator (service layer).
//!
//! Resolves the active router and routes app network exposure to either the
//! built-in nginx generator or a router *plugin* via the
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

use crate::config::router::Router;
use crate::config::RikuPaths;
use crate::util::backend::Backend;
use crate::util::domain::parse_domains;
use crate::util::flag::https_only;
use crate::util::route_plan::{HttpsPolicy, RoutePlan};

mod backends;
mod caddy;
mod fallback;
mod plugin;
mod reload;

pub(crate) use backends::web_copies;
pub use backends::{copy_socket, current_backends};
use caddy::configure_caddy;
use fallback::fallback_backends;
use plugin::{configure_plugin, remove_plugin};
use reload::record_outcome;
pub use reload::{reload, Baseline, ERROR_FILE as RELOAD_ERROR_FILE};

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
/// Procfile app to its web copies, or to its old default backend when it has
/// none.
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

/// Configure the active router so `app` is reachable, with a backend for
/// every web copy it has today, and return once the router confirmed the new
/// config. `engine` is `Some` for a container app.
#[allow(clippy::question_mark)]
pub fn configure(
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
    engine: Option<&Engine>,
) -> Result<()> {
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

/// Render and write `app`'s router config for `backends`, then reload the
/// active router and wait for it to confirm.
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
    let baseline = match active {
        Router::Nginx => Baseline::capture(&paths.reload_programs),
        Router::Caddy | Router::Plugin(_) => Baseline::Direct,
    };
    let written = match active {
        Router::Nginx => crate::nginx::generate_nginx_config(app, app_path, env, backends, paths),
        Router::Caddy => configure_caddy(app, env, backends, paths),
        Router::Plugin(name) => configure_plugin(name, app, env, backends, paths),
    };
    if let Err(e) = written {
        return Err(e);
    }
    reload(active, paths, &baseline)
}

/// Remove `app` from the active router.
///
/// nginx removal is built-in. For a plugin router this dispatches `unconfigure`
/// (the app's config is keyed by `RIKU_APP`, which is still available even
/// though the app's `ENV` is already gone by destroy time) then `reload`.
///
/// `unconfigure` is **best-effort**: it was added within API v1, so a plugin
/// predating it may not implement the verb and will exit non-zero, in that
/// case the following `reload` lets the plugin reconcile the orphaned upstream
/// on its own. See `PLUGIN_PROTOCOL.md` §6.2.
#[allow(clippy::question_mark)]
pub fn remove(app: &str, paths: &RikuPaths) -> Result<()> {
    let active = match Router::load(paths) {
        Ok(router) => router,
        Err(e) => return Err(e),
    };

    let outcome = match &active {
        Router::Nginx => crate::nginx::remove_nginx_config(app, paths),
        Router::Caddy => match crate::caddy::remove_site(app, paths) {
            Ok(()) => reload(&active, paths, &Baseline::Direct),
            Err(e) => Err(e),
        },
        Router::Plugin(name) => remove_plugin(name, app, paths),
    };
    record_outcome(paths, &outcome);
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
    })
}

#[cfg(test)]
#[path = "router_tests.rs"]
mod tests;
