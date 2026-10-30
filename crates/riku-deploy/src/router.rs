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
use std::net::IpAddr;
use std::path::Path;

use anyhow::{anyhow, Result};

use crate::config::router::Router;
use crate::config::RikuPaths;
use crate::util::backend::Backend;
use crate::util::domain::parse_domains;
use crate::util::flag::https_only;
use crate::util::route_plan::{HttpsPolicy, RoutePlan};

mod caddy;
mod plugin;

use caddy::configure_caddy;
use plugin::{configure_plugin, remove_plugin};

/// Configure the active router so `app` is reachable.
///
/// With the default `nginx` router this generates the config and returns; the
/// supervisor daemon performs the graceful `nginx -s reload` when it observes
/// the changed config (unchanged legacy behavior). With a plugin router there
/// is no daemon watcher, so this dispatches `configure` then `reload`.
#[allow(clippy::question_mark)]
pub fn configure(
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
) -> Result<()> {
    let active = match Router::load(paths) {
        Ok(router) => router,
        Err(e) => return Err(e),
    };

    match active {
        Router::Nginx => {
            let backends = match crate::nginx::default_backends(app, env, paths) {
                Ok(backends) => backends,
                Err(e) => return Err(anyhow!("cannot route '{app}' through nginx: {e}")),
            };
            crate::nginx::generate_nginx_config(app, app_path, env, &backends, paths)
        }
        Router::Caddy => configure_caddy(app, env, paths),
        Router::Plugin(name) => configure_plugin(&name, app, env, paths),
    }
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

    match active {
        Router::Nginx => crate::nginx::remove_nginx_config(app, paths),
        Router::Caddy => crate::caddy::remove_site(app, paths),
        Router::Plugin(name) => remove_plugin(&name, app, paths),
    }
}

/// The routing plan for an app, built from its ENV, with one backend at `host`
/// on the app's upstream port. Until the supervisor reports its copies that
/// single port is all there is.
///
/// A malformed `RIKU_DOMAINS`, or a port key that is set but not a port
/// number, is an error, so nothing unvalidated reaches a router.
#[allow(clippy::question_mark)]
fn route_plan(env: &HashMap<String, String>, host: IpAddr) -> Result<RoutePlan> {
    let raw = env
        .get("RIKU_DOMAINS")
        .map(String::as_str)
        .unwrap_or_default();
    let domains = match parse_domains(raw) {
        Ok(domains) => domains,
        Err(e) => return Err(anyhow!("invalid RIKU_DOMAINS: {e}")),
    };
    let port = match crate::nginx::upstream_port_number(env) {
        Ok(port) => port,
        Err(e) => return Err(e),
    };
    let https = if https_only(env) {
        HttpsPolicy::Only
    } else {
        HttpsPolicy::AlsoPlainHttp
    };
    Ok(RoutePlan {
        domains,
        https,
        backends: vec![Backend::Tcp { host, port }],
    })
}

#[cfg(test)]
#[path = "router_tests.rs"]
mod tests;
