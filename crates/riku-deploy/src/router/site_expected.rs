//! Whether the active router keeps a site file of `app`'s own, so `riku
//! doctor --fix` redeploys only apps whose site is really missing.

use std::collections::HashMap;

use anyhow::Result;

use super::app_routing::{AppRouting, RoutingMode};
use super::routing_env::{routing_env, Record};
use crate::config::router::Router;
use crate::config::RikuPaths;
use crate::supervisor_ctl::read_app_env;

/// Path mode drops the app's own site for the shared path site. Caddy
/// removes the site of an app with no domain and no port (`caddy.rs`),
/// while nginx serves such an app under its catch all name.
fn site_for(routing: &AppRouting, active: &Router) -> bool {
    if routing.mode == RoutingMode::Path {
        return false;
    }
    match active {
        Router::Caddy => !routing.domains.is_empty() || routing.port.is_some(),
        Router::Nginx | Router::Plugin(_) => true,
    }
}

/// A Procfile app the deploy takes off the router (see
/// [`super::has_route`]). Container apps route through their engine.
fn procfile_app_without_route(
    app: &str,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
) -> Result<bool> {
    match crate::containers::is_container_app(&paths.app_root.join(app), env) {
        Ok(true) => return Ok(false),
        Ok(false) => {}
        Err(e) => return Err(e),
    }
    match super::has_route(app, env, paths) {
        Ok(routed) => Ok(!routed),
        Err(e) => Err(e),
    }
}

/// Whether `app` should have a site file under the active router. Reads the
/// app's ENV and routing state; changes nothing.
#[allow(clippy::question_mark)]
pub fn site_expected(app: &str, paths: &RikuPaths) -> Result<bool> {
    let active = match Router::load(paths) {
        Ok(active) => active,
        Err(e) => return Err(e),
    };
    let env = match read_app_env(app, paths) {
        Ok(env) => env,
        Err(e) => return Err(e),
    };
    match procfile_app_without_route(app, &env, paths) {
        Ok(true) => return Ok(false),
        Ok(false) => {}
        Err(e) => return Err(e),
    }
    match routing_env(app, &env, paths, &active, Record::Peek) {
        Ok((_, routing)) => Ok(site_for(&routing, &active)),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
#[path = "site_expected_tests.rs"]
mod tests;
