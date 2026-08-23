//! Router seam orchestrator (service layer).
//!
//! Resolves the active router and routes app network exposure to either the
//! built-in nginx generator or a router *plugin* via the
//! [`crate::plugins::router`] seam. Callers (deploy, destroy) go through here
//! instead of reaching into nginx directly, so swapping the router is a config
//! change, not a code change. See `PLUGIN_PROTOCOL.md` §6.2.
//!
//! The router is a **host-level singleton**, chosen by the `RIKU_ROUTER`
//! environment variable (default `nginx`) — not a per-app setting. Per-app
//! `ENV` still shapes the *contents* of a router config (domains, port, TLS),
//! but never which router is active.
//!
//! Security: the request handed to a plugin is built only from a fixed set of
//! the app's own `ENV` keys, serialized as JSON — no shell interpolation, no
//! caller-controlled argv. The plugin runs under the shared plugin timeout.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Serialize;

use crate::config::RikuPaths;
use crate::plugins::{bundles, router};

/// The built-in router; also the default when `RIKU_ROUTER` is unset.
const DEFAULT_ROUTER: &str = "nginx";

/// The request handed to a router plugin's `configure` verb (one JSON line on
/// stdin). Mirrors `PLUGIN_PROTOCOL.md` §6.2.
#[derive(Serialize)]
struct ConfigureRequest {
    app: String,
    domains: Vec<String>,
    upstream_port: u16,
    https: bool,
}

/// The active router name: host-level `RIKU_ROUTER`, else the built-in default.
fn active_router() -> String {
    std::env::var("RIKU_ROUTER")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_ROUTER.to_string())
}

/// Configure the active router so `app` is reachable.
///
/// With the default `nginx` router this generates the config and returns; the
/// supervisor daemon performs the graceful `nginx -s reload` when it observes
/// the changed config (unchanged legacy behavior). With a plugin router there
/// is no daemon watcher, so this dispatches `configure` then `reload`.
pub fn configure(
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
) -> Result<()> {
    let router_name = active_router();
    if router_name == DEFAULT_ROUTER {
        return crate::nginx::generate_nginx_config(app, app_path, env, paths);
    }

    let (bundle, manifest) = bundles::find_router(&paths.plugin_root, &router_name)
        .with_context(|| format!("router plugin '{router_name}' not installed"))?;

    let request = ConfigureRequest {
        app: app.to_string(),
        domains: domains_from_env(env),
        upstream_port: upstream_port_from_env(env),
        https: https_from_env(env),
    };
    let request = serde_json::to_value(&request)?;
    router::run_verb(
        paths,
        &bundle,
        &manifest,
        "configure",
        Some(app),
        Some(&request),
    )?;
    router::run_verb(paths, &bundle, &manifest, "reload", None, None)?;
    Ok(())
}

/// Remove `app` from the active router.
///
/// nginx removal is built-in. For a plugin router this dispatches `unconfigure`
/// (the app's config is keyed by `RIKU_APP`, which is still available even
/// though the app's `ENV` is already gone by destroy time) then `reload`.
///
/// `unconfigure` is **best-effort**: it was added within API v1, so a plugin
/// predating it may not implement the verb and will exit non-zero — in that
/// case the following `reload` lets the plugin reconcile the orphaned upstream
/// on its own. See `PLUGIN_PROTOCOL.md` §6.2.
pub fn remove(app: &str, paths: &RikuPaths) -> Result<()> {
    let router_name = active_router();
    if router_name == DEFAULT_ROUTER {
        return crate::nginx::remove_nginx_config(app, paths);
    }
    if let Some((bundle, manifest)) = bundles::find_router(&paths.plugin_root, &router_name) {
        if let Err(e) = router::run_verb(paths, &bundle, &manifest, "unconfigure", Some(app), None)
        {
            tracing::warn!(router = %router_name, "unconfigure failed, relying on reload: {e}");
        }
        router::run_verb(paths, &bundle, &manifest, "reload", None, None)?;
    }
    Ok(())
}

/// Domains for the app, parsed from `NGINX_SERVER_NAME` (space/comma separated).
fn domains_from_env(env: &HashMap<String, String>) -> Vec<String> {
    env.get("NGINX_SERVER_NAME")
        .map(|s| {
            s.split([',', ' '])
                .map(str::trim)
                .filter(|d| !d.is_empty())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

/// The app's upstream port: `NGINX_INTERNAL_PORT`, then `PORT`, default 8080.
fn upstream_port_from_env(env: &HashMap<String, String>) -> u16 {
    env.get("NGINX_INTERNAL_PORT")
        .or_else(|| env.get("PORT"))
        .and_then(|p| p.trim().parse().ok())
        .unwrap_or(8080)
}

/// Whether HTTPS-only is requested (`NGINX_HTTPS_ONLY` set to a truthy value).
fn https_from_env(env: &HashMap<String, String>) -> bool {
    env.get("NGINX_HTTPS_ONLY")
        .map(|v| {
            let v = v.trim().to_ascii_lowercase();
            !matches!(v.as_str(), "" | "0" | "false" | "no")
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn domains_split_on_space_and_comma() {
        let e = env(&[("NGINX_SERVER_NAME", "a.com, b.com  c.com")]);
        assert_eq!(domains_from_env(&e), vec!["a.com", "b.com", "c.com"]);
        assert!(domains_from_env(&env(&[])).is_empty());
    }

    #[test]
    fn upstream_port_prefers_internal_then_port_then_default() {
        assert_eq!(
            upstream_port_from_env(&env(&[("NGINX_INTERNAL_PORT", "9000"), ("PORT", "3000")])),
            9000
        );
        assert_eq!(upstream_port_from_env(&env(&[("PORT", "3000")])), 3000);
        assert_eq!(upstream_port_from_env(&env(&[])), 8080);
        // Garbage falls back to the default rather than panicking.
        assert_eq!(upstream_port_from_env(&env(&[("PORT", "nope")])), 8080);
    }

    #[test]
    fn https_truthiness() {
        assert!(https_from_env(&env(&[("NGINX_HTTPS_ONLY", "1")])));
        assert!(https_from_env(&env(&[("NGINX_HTTPS_ONLY", "true")])));
        assert!(!https_from_env(&env(&[("NGINX_HTTPS_ONLY", "false")])));
        assert!(!https_from_env(&env(&[("NGINX_HTTPS_ONLY", "0")])));
        assert!(!https_from_env(&env(&[])));
    }
}
