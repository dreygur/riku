//! Router plugin branch of the router seam (`PLUGIN_PROTOCOL.md` section 6.2).
//!
//! Security: the request is built from a [`RoutePlan`], so domains are already
//! validated, and is sent as JSON on the plugin's stdin with no shell
//! interpolation. A plugin that does not declare router protocol 2 is refused
//! before `configure` runs, so an old plugin never receives a request it would
//! misread. Removal sends `unconfigure` and `reload`, which carry no request,
//! to a plugin of any protocol.

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use riku_bundle::{PluginManifest, ROUTER_PROTOCOL};

use super::route_plan;
use crate::config::RikuPaths;
use crate::plugins::{bundles, router};
use crate::util::backend::Backend;
use crate::util::route_plan::{HttpsPolicy, RoutePlan};

fn backend_json(backend: &Backend) -> serde_json::Value {
    match backend {
        Backend::Tcp { host, port } => {
            serde_json::json!({"kind": "tcp", "host": host.to_string(), "port": port})
        }
        Backend::Unix(path) => {
            serde_json::json!({"kind": "unix", "path": path.display().to_string()})
        }
    }
}

/// The JSON a router plugin receives on stdin for its `configure` verb.
fn configure_request(app: &str, plan: &RoutePlan) -> serde_json::Value {
    serde_json::json!({
        "protocol": ROUTER_PROTOCOL,
        "app": app,
        "domains": plan.domains.iter().map(|d| d.as_str()).collect::<Vec<_>>(),
        "https": plan.https == HttpsPolicy::Only,
        "backends": plan.backends.iter().map(backend_json).collect::<Vec<_>>(),
    })
}

/// The refusal for a plugin that does not speak [`ROUTER_PROTOCOL`], if any.
/// A manifest without a `[router]` table counts as protocol 1.
fn protocol_refusal(manifest: &PluginManifest) -> Option<anyhow::Error> {
    let spoken = manifest
        .router
        .as_ref()
        .map_or(1, |section| section.protocol);
    if spoken == ROUTER_PROTOCOL {
        return None;
    }
    Some(anyhow!(
        "router plugin '{}' speaks router protocol {spoken}; this riku needs protocol {ROUTER_PROTOCOL}. Update the plugin (PLUGIN_PROTOCOL.md section 6.2)",
        manifest.name
    ))
}

/// Send `configure` for `app` to the router plugin `name`, routing to
/// `backends`.
///
/// Fails before running any verb when the plugin does not speak the current
/// router protocol, or when the app's `RIKU_DOMAINS` is invalid.
#[allow(clippy::question_mark)]
pub(super) fn configure_plugin(
    name: &str,
    app: &str,
    env: &HashMap<String, String>,
    backends: &[Backend],
    paths: &RikuPaths,
) -> Result<()> {
    let (bundle, manifest) = match bundles::find_router(&paths.plugin_root, name) {
        Some(found) => found,
        None => return Err(anyhow!("router plugin '{name}' not installed")),
    };
    if let Some(refusal) = protocol_refusal(&manifest) {
        return Err(refusal);
    }

    let plan = match route_plan(env, backends) {
        Ok(plan) => plan,
        Err(e) => return Err(anyhow!("cannot route '{app}' through '{name}': {e}")),
    };
    let request = configure_request(app, &plan);

    router::run_verb(
        paths,
        &bundle,
        &manifest,
        "configure",
        Some(app),
        Some(&request),
    )
}

/// Send `reload` to the router plugin `name`.
pub(super) fn reload_plugin(name: &str, paths: &RikuPaths) -> Result<()> {
    let (bundle, manifest) = match bundles::find_router(&paths.plugin_root, name) {
        Some(found) => found,
        None => return Err(anyhow!("router plugin '{name}' not installed")),
    };
    router::run_verb(paths, &bundle, &manifest, "reload", None, None)
}

/// Send `unconfigure` then `reload`. `unconfigure` is best effort for every
/// protocol: a plugin that lacks the verb is reconciled by the reload.
pub(super) fn remove_plugin(name: &str, app: &str, paths: &RikuPaths) -> Result<()> {
    let (bundle, manifest) = match bundles::find_router(&paths.plugin_root, name) {
        Some(found) => found,
        None => return Ok(()),
    };

    if let Err(e) = router::run_verb(paths, &bundle, &manifest, "unconfigure", Some(app), None) {
        tracing::warn!(router = %name, "unconfigure failed, relying on reload: {e}");
    }
    router::run_verb(paths, &bundle, &manifest, "reload", None, None)
}

#[cfg(test)]
#[path = "plugin_tests.rs"]
mod tests;
