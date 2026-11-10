//! nginx branch of the router seam: the deploy user prepares what only it
//! may touch (static copies, certificate files), builds the route and asks
//! the router helper to install it.
//!
//! Security: nothing here writes a router config or runs nginx. The route
//! goes through the `riku_route` gate in `build_route` for an early error
//! and again in the helper, and the helper's answer is data: a refusal is
//! reported with its stage and message, never acted on. Domains reach
//! acme.sh and openssl only after the same validation the route uses.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{anyhow, Result};
use riku_container::types::AppName;
use riku_route::{Reply, Request, RouterKind};

use super::app_routing::AppRouting;
use super::fallback_tls::fallback_certificate;
use super::route_build::{build_route, nginx_domains, CATCH_ALL_NAME};
use super::route_env::tls;
use super::routing_state::TlsState;
use crate::config::router::Router;
use crate::config::RikuPaths;
use crate::util::backend::Backend;
use crate::util::echo;
use crate::util::flag::https_only;

/// The backend a check renders with: the route gate never connects to it.
const STAND_IN_PORT: u16 = 1;
const HTTPS_ONLY_KEY: &str = "RIKU_HTTPS_ONLY";

pub(super) fn app_name(app: &str) -> Result<AppName> {
    match AppName::try_from(app) {
        Ok(name) => Ok(name),
        Err(e) => Err(anyhow!("cannot route '{app}': {e}")),
    }
}

/// Turn the helper's answer to `action` into a result. A refusal keeps the
/// stage that gave it, since a schema refusal and a failed reload are fixed
/// in different places.
pub(super) fn helper_outcome(reply: Reply, action: &str) -> Result<bool> {
    match reply {
        Reply::Ok { reloaded } => Ok(reloaded),
        Reply::Failed { stage, error } => Err(anyhow!(
            "the router helper could not {action} (stage {stage}): {error}"
        )),
        Reply::Status { .. } => Err(anyhow!(
            "the router helper answered a status to a request to {action}"
        )),
    }
}

fn certificate_domains(app: &AppName, env: &HashMap<String, String>) -> Result<Vec<String>> {
    match nginx_domains(app, env) {
        Ok(domains) => Ok(domains
            .into_iter()
            .filter(|domain| domain != CATCH_ALL_NAME)
            .collect()),
        Err(e) => Err(e),
    }
}

/// An https only app needs a pair to send, and the first deploy has none yet:
/// issue it now. An app without `RIKU_DOMAINS` has no name to issue for.
#[allow(clippy::question_mark)]
fn ensure_certificates(
    app: &AppName,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
) -> Result<()> {
    if !https_only(env) || !env.contains_key("RIKU_DOMAINS") {
        return Ok(());
    }
    let domains = match certificate_domains(app, env) {
        Ok(domains) => domains,
        Err(e) => return Err(e),
    };
    if domains.is_empty() {
        return Ok(());
    }
    match riku_nginx::ensure_ssl_certificates(app.as_str(), &domains, paths) {
        Ok(_) => Ok(()),
        Err(e) => Err(anyhow!("cannot get a certificate for '{app}': {e}")),
    }
}

fn nginx_route(
    app: &AppName,
    env: &HashMap<String, String>,
    backends: &[Backend],
    paths: &RikuPaths,
) -> Result<riku_route::Route> {
    match build_route(app, env, backends, paths, &Router::Nginx) {
        Ok(Some(route)) => Ok(route),
        Ok(None) => Err(anyhow!("'{app}' has no nginx route")),
        Err(e) => Err(e),
    }
}

/// Build `app`'s route from `env` and have the helper install it. A
/// fallback domain carries the pair in `nginx_root` when there is one, so
/// nginx serves it on 443 as well as plain HTTP.
#[allow(clippy::question_mark)]
fn install(
    send: &impl Fn(&Request) -> Result<Reply>,
    name: &AppName,
    env: &HashMap<String, String>,
    fallback: bool,
    backends: &[Backend],
    paths: &RikuPaths,
) -> Result<()> {
    let mut route = match nginx_route(name, env, backends, paths) {
        Ok(route) => route,
        Err(e) => return Err(e),
    };
    if fallback && route.tls.is_none() {
        route.tls = match tls(name, paths) {
            Ok(found) => found,
            Err(e) => return Err(anyhow!("tls: {e}")),
        };
    }
    let request = Request::Apply {
        app: name.to_string(),
        router: RouterKind::Nginx,
        route: Box::new(route),
    };
    match send(&request) {
        Ok(reply) => helper_outcome(reply, "install the site").map(|_| ()),
        Err(e) => Err(e),
    }
}

fn has_pair(name: &AppName, paths: &RikuPaths) -> bool {
    matches!(tls(name, paths), Ok(Some(_)))
}

/// Publish `app`'s static files and certificates, then have the helper
/// install its site and reload nginx. Returns once nginx accepted the reload.
///
/// A site that wants a certificate it does not have yet is first installed
/// over plain HTTP, so the ACME challenge reaches it, then the certificate
/// is issued and the site installed again with it.
#[allow(clippy::question_mark)]
pub(super) fn apply_nginx_site(
    send: impl Fn(&Request) -> Result<Reply>,
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    routing: &AppRouting,
    backends: &[Backend],
    paths: &RikuPaths,
) -> Result<()> {
    let name = match app_name(app) {
        Ok(name) => name,
        Err(e) => return Err(e),
    };
    if backends.is_empty() {
        return Err(anyhow!("no backends to route '{app}' to"));
    }
    riku_nginx::warn_about_app_nginx_configs(app, app_path);
    if let Err(e) = riku_nginx::publish_static(app, app_path, env, paths) {
        return Err(anyhow!("cannot publish static files for '{app}': {e}"));
    }
    let own_tls = https_only(env) && env.contains_key("RIKU_DOMAINS");
    let fallback_domain = routing.domains.first().filter(|_| routing.fallback);
    if (own_tls || fallback_domain.is_some()) && !has_pair(&name, paths) {
        let mut plain = env.clone();
        plain.remove(HTTPS_ONLY_KEY);
        if let Err(e) = install(&send, &name, &plain, false, backends, paths) {
            return Err(e);
        }
        if let Some(domain) = fallback_domain {
            if fallback_certificate(app, domain, paths) != TlsState::Issued {
                echo(
                    &format!("-----> HTTPS for {domain} is not available yet; the app answers on plain HTTP"),
                    "yellow",
                );
                return Ok(());
            }
        }
    }
    if own_tls {
        if let Err(e) = ensure_certificates(&name, env, paths) {
            return Err(e);
        }
    }
    install(
        &send,
        &name,
        env,
        fallback_domain.is_some(),
        backends,
        paths,
    )
}

/// Refuse a deploy before it starts a copy when `app`'s route would fail the
/// gate or the helper is not answering. `nginx -t` itself runs in the helper
/// at apply time.
#[allow(clippy::question_mark)]
pub(super) fn check_nginx_site(
    send: impl Fn(&Request) -> Result<Reply>,
    app: &str,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
) -> Result<()> {
    let name = match app_name(app) {
        Ok(name) => name,
        Err(e) => return Err(e),
    };
    let stand_in = [Backend::loopback(STAND_IN_PORT)];
    if let Err(e) = nginx_route(&name, env, &stand_in, paths) {
        return Err(e);
    }
    match send(&Request::Status {}) {
        Ok(Reply::Status { .. }) => Ok(()),
        Ok(other) => Err(anyhow!("the router helper answered {other:?} to a status")),
        Err(e) => Err(e),
    }
}

/// Take `app` out of nginx through the helper, then delete the certificate
/// files and sockets the deploy user holds. The local files stay when the
/// helper fails, so a retried removal finds the app as it was. Returns whether
/// nginx was reloaded.
#[allow(clippy::question_mark)]
pub(super) fn remove_nginx_site(
    send: impl Fn(&Request) -> Result<Reply>,
    app: &str,
    paths: &RikuPaths,
) -> Result<bool> {
    let name = match app_name(app) {
        Ok(name) => name,
        Err(e) => return Err(e),
    };
    let reloaded = match send(&Request::Remove {
        app: name.to_string(),
        router: RouterKind::Nginx,
    }) {
        Ok(reply) => match helper_outcome(reply, "remove the site") {
            Ok(reloaded) => reloaded,
            Err(e) => return Err(e),
        },
        Err(e) => return Err(e),
    };
    match riku_nginx::remove_deploy_files(app, paths) {
        Ok(()) => Ok(reloaded),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
#[path = "nginx_site_tests.rs"]
mod tests;
