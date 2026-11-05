//! Caddy branch of the router seam: turns an app's ENV into a route and asks
//! the router helper to install or clear its site.
//!
//! Security: nothing here writes a site file or runs Caddy. Domains go through
//! [`parse_domains`], so a value like `a.com {` fails the deploy, and the
//! route passes the `riku_route` Caddy gate here and again in the helper. The
//! helper's answer is data: a refusal is reported with its stage, never
//! acted on.

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use riku_route::{Reply, Request, RouterKind};

use super::nginx_site::{app_name, helper_outcome};
use super::route_build::build_route;
use super::route_plan;
use crate::config::router::Router;
use crate::config::RikuPaths;
use crate::util::backend::Backend;
use crate::util::echo;
use crate::util::route_plan::RoutePlan;

const DOMAINS_KEY: &str = "RIKU_DOMAINS";
const NGINX_WSGI_KEY: &str = "RIKU_NGINX_WSGI";
const NGINX_STATIC_KEY: &str = "RIKU_NGINX_STATIC";

/// The Caddy routing plan for an app, or `None` when it sets no domain.
///
/// Caddy only proxies HTTP over TCP, so uWSGI socket apps and nginx static
/// sites are rejected instead of getting a site that routes to nothing.
#[allow(clippy::question_mark)]
fn caddy_request(env: &HashMap<String, String>, backends: &[Backend]) -> Result<Option<RoutePlan>> {
    let tcp_only = [NGINX_WSGI_KEY, NGINX_STATIC_KEY]
        .iter()
        .find(|key| env.contains_key(**key));
    if let Some(key) = tcp_only {
        return Err(anyhow!(
            "{key} is set, but Caddy proxies TCP only. Run a web: process listening on $PORT"
        ));
    }

    let plan = match route_plan(env, backends) {
        Ok(plan) => plan,
        Err(e) => return Err(e),
    };
    if plan.domains.is_empty() {
        return Ok(None);
    }
    Ok(Some(plan))
}

/// Have the router helper write `app`'s Caddy site and reload Caddy, or take
/// the site out when the app sets no domain. Returns once Caddy accepted the
/// reload.
#[allow(clippy::question_mark)]
pub(super) fn apply_caddy_site(
    send: impl Fn(&Request) -> Result<Reply>,
    app: &str,
    env: &HashMap<String, String>,
    backends: &[Backend],
    paths: &RikuPaths,
) -> Result<()> {
    let name = match app_name(app) {
        Ok(name) => name,
        Err(e) => return Err(e),
    };
    let plan = match caddy_request(env, backends) {
        Ok(plan) => plan,
        Err(e) => return Err(e),
    };
    if plan.is_none() {
        echo(
            &format!("-----> No {DOMAINS_KEY} set, app is not routed. Set one with: riku config set {app} {DOMAINS_KEY}=<domain>"),
            "yellow",
        );
        return remove_caddy_site(send, app).map(|_| ());
    }
    let route = match build_route(&name, env, backends, paths, &Router::Caddy) {
        Ok(Some(route)) => route,
        Ok(None) => return Err(anyhow!("'{app}' has no Caddy route")),
        Err(e) => return Err(e),
    };
    let request = Request::Apply {
        app: name.to_string(),
        router: RouterKind::Caddy,
        route: Box::new(route),
    };
    match send(&request) {
        Ok(reply) => helper_outcome(reply, "install the site").map(|_| ()),
        Err(e) => Err(e),
    }
}

/// Take `app`'s Caddy site out through the helper. Returns whether Caddy was
/// reloaded.
#[allow(clippy::question_mark)]
pub(super) fn remove_caddy_site(
    send: impl Fn(&Request) -> Result<Reply>,
    app: &str,
) -> Result<bool> {
    let name = match app_name(app) {
        Ok(name) => name,
        Err(e) => return Err(e),
    };
    let request = Request::Remove {
        app: name.to_string(),
        router: RouterKind::Caddy,
    };
    match send(&request) {
        Ok(reply) => helper_outcome(reply, "remove the site"),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
#[path = "caddy_tests.rs"]
mod tests;
