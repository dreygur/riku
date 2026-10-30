//! Caddy branch of the router seam: turns an app's ENV into a site request
//! and writes or clears its site file.
//!
//! Security: every value reaching the site file is typed. Domains go through
//! [`parse_domains`], so a value like `a.com {` fails the deploy instead of
//! injecting Caddyfile syntax.

use std::collections::HashMap;

use anyhow::{anyhow, Result};

use super::upstream_port_from_env;
use crate::caddy::{HttpsPolicy, SiteRequest};
use crate::config::RikuPaths;
use crate::util::domain::parse_domains;
use crate::util::echo;
use crate::util::flag::https_only;

const DOMAINS_KEY: &str = "RIKU_DOMAINS";
const NGINX_WSGI_KEY: &str = "RIKU_NGINX_WSGI";
const NGINX_STATIC_KEY: &str = "RIKU_NGINX_STATIC";

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

    let https = if https_only(env) {
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
pub(super) fn configure_caddy(
    app: &str,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
) -> Result<()> {
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

#[cfg(test)]
#[path = "caddy_tests.rs"]
mod tests;
