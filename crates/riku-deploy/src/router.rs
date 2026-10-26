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
use serde::Serialize;

use crate::caddy::{HttpsPolicy, SiteRequest};
use crate::config::router::Router;
use crate::config::RikuPaths;
use crate::plugins::{bundles, router};
use crate::util::domain::parse_domains;
use crate::util::echo;

const DOMAINS_KEY: &str = "RIKU_DOMAINS";
const NGINX_WSGI_KEY: &str = "RIKU_NGINX_WSGI";
const NGINX_STATIC_KEY: &str = "RIKU_NGINX_STATIC";

/// The request handed to a router plugin's `configure` verb (one JSON line on
/// stdin). Mirrors `PLUGIN_PROTOCOL.md` §6.2.
#[derive(Serialize)]
struct ConfigureRequest {
    app: String,
    domains: Vec<String>,
    upstream_port: u16,
    https: bool,
}

#[allow(clippy::question_mark)]
fn configure_plugin(
    name: &str,
    app: &str,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
) -> Result<()> {
    let (bundle, manifest) = match bundles::find_router(&paths.plugin_root, name) {
        Some(found) => found,
        None => return Err(anyhow!("router plugin '{name}' not installed")),
    };

    let request = ConfigureRequest {
        app: app.to_string(),
        domains: domains_from_env(env),
        upstream_port: upstream_port_from_env(env),
        https: https_from_env(env),
    };
    let request = match serde_json::to_value(&request) {
        Ok(value) => value,
        Err(e) => return Err(anyhow!("failed to encode router request: {e}")),
    };

    if let Err(e) = router::run_verb(
        paths,
        &bundle,
        &manifest,
        "configure",
        Some(app),
        Some(&request),
    ) {
        return Err(e);
    }
    router::run_verb(paths, &bundle, &manifest, "reload", None, None)
}

/// The Caddy site for an app, or `None` when it sets no domain.
///
/// Caddy only proxies HTTP over TCP, so uWSGI socket apps and nginx static
/// sites are rejected instead of getting a site that routes to nothing.
#[allow(clippy::question_mark)]
fn caddy_request(env: &HashMap<String, String>) -> Result<Option<SiteRequest>> {
    let tcp_only = [NGINX_WSGI_KEY, NGINX_STATIC_KEY]
        .iter()
        .find(|key| env.contains_key(**key));
    if let Some(key) = tcp_only {
        return Err(anyhow!(
            "{key} is set, but Caddy proxies TCP only. Run a web: process listening on $PORT"
        ));
    }

    let raw = env.get(DOMAINS_KEY).map(String::as_str).unwrap_or_default();
    let domains = match parse_domains(raw) {
        Ok(domains) => domains,
        Err(e) => return Err(e),
    };
    if domains.is_empty() {
        return Ok(None);
    }

    let https = if https_from_env(env) {
        HttpsPolicy::Only
    } else {
        HttpsPolicy::AlsoPlainHttp
    };
    Ok(Some(SiteRequest {
        domains,
        upstream_port: upstream_port_from_env(env),
        https,
    }))
}

/// Write or clear `app`'s Caddy site. The `riku-caddy.path` unit reloads
/// Caddy when the site directory changes, so there is no reload call here.
#[allow(clippy::question_mark)]
fn configure_caddy(app: &str, env: &HashMap<String, String>, paths: &RikuPaths) -> Result<()> {
    let site = match caddy_request(env) {
        Ok(site) => site,
        Err(e) => return Err(e),
    };

    match site {
        Some(site) => crate::caddy::write_site(app, &site, paths),
        None => {
            echo(
                &format!("-----> No {DOMAINS_KEY} set, app is not routed. Set one with: riku config set {app} {DOMAINS_KEY}=<domain>"),
                "yellow",
            );
            crate::caddy::remove_site(app, paths)
        }
    }
}

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
        Router::Nginx => crate::nginx::generate_nginx_config(app, app_path, env, paths),
        Router::Caddy => configure_caddy(app, env, paths),
        Router::Plugin(name) => configure_plugin(&name, app, env, paths),
    }
}

fn remove_plugin(name: &str, app: &str, paths: &RikuPaths) -> Result<()> {
    let (bundle, manifest) = match bundles::find_router(&paths.plugin_root, name) {
        Some(found) => found,
        None => return Ok(()),
    };

    if let Err(e) = router::run_verb(paths, &bundle, &manifest, "unconfigure", Some(app), None) {
        tracing::warn!(router = %name, "unconfigure failed, relying on reload: {e}");
    }
    router::run_verb(paths, &bundle, &manifest, "reload", None, None)
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

/// Domains for the app, parsed from `RIKU_DOMAINS` (space/comma separated).
fn domains_from_env(env: &HashMap<String, String>) -> Vec<String> {
    env.get("RIKU_DOMAINS")
        .map(|s| {
            s.split([',', ' '])
                .map(str::trim)
                .filter(|d| !d.is_empty())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

/// The app's upstream port: `RIKU_UPSTREAM_PORT`, then `PORT`, default 8080.
fn upstream_port_from_env(env: &HashMap<String, String>) -> u16 {
    env.get("RIKU_UPSTREAM_PORT")
        .or_else(|| env.get("PORT"))
        .and_then(|p| p.trim().parse().ok())
        .unwrap_or(8080)
}

/// Whether HTTPS-only is requested (`RIKU_HTTPS_ONLY` set to a truthy value).
fn https_from_env(env: &HashMap<String, String>) -> bool {
    env.get("RIKU_HTTPS_ONLY")
        .map(|v| {
            let v = v.trim().to_ascii_lowercase();
            !matches!(v.as_str(), "" | "0" | "false" | "no")
        })
        .unwrap_or(false)
}

#[cfg(test)]
#[path = "router_tests.rs"]
mod tests;
