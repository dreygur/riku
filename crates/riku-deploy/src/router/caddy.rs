//! Caddy branch of the router seam: turns an app's ENV into a site request
//! and writes or clears its site file.
//!
//! Security: every value reaching the site file is typed. Domains go through
//! [`parse_domains`], so a value like `a.com {` fails the deploy instead of
//! injecting Caddyfile syntax.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr};

use anyhow::{anyhow, Result};

use super::route_plan;
use crate::config::RikuPaths;
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
fn caddy_request(env: &HashMap<String, String>) -> Result<Option<RoutePlan>> {
    let tcp_only = [NGINX_WSGI_KEY, NGINX_STATIC_KEY]
        .iter()
        .find(|key| env.contains_key(**key));
    if let Some(key) = tcp_only {
        return Err(anyhow!(
            "{key} is set, but Caddy proxies TCP only. Run a web: process listening on $PORT"
        ));
    }

    // Caddy reaches the app over loopback whatever RIKU_BIND_ADDRESS says.
    let plan = match route_plan(env, IpAddr::V4(Ipv4Addr::LOCALHOST)) {
        Ok(plan) => plan,
        Err(e) => return Err(e),
    };
    if plan.domains.is_empty() {
        return Ok(None);
    }
    Ok(Some(plan))
}

/// Write or clear `app`'s Caddy site. The `riku-caddy.path` unit reloads
/// Caddy when the site directory changes, so there is no reload call here.
#[allow(clippy::question_mark)]
pub(super) fn configure_caddy(
    app: &str,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
) -> Result<()> {
    let plan = match caddy_request(env) {
        Ok(plan) => plan,
        Err(e) => return Err(e),
    };

    match plan {
        Some(plan) => crate::caddy::write_site(app, &plan, paths),
        None => {
            echo(
                &format!("-----> No {DOMAINS_KEY} set, app is not routed. Set one with: riku config set {app} {DOMAINS_KEY}=<domain>"),
                "yellow",
            );
            crate::caddy::remove_site(app, paths)
        }
    }
}

#[cfg(test)]
#[path = "caddy_tests.rs"]
mod tests;
