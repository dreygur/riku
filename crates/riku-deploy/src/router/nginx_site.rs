//! nginx branch of the router seam: the deploy user prepares what only it
//! may touch (static copies, certificate files), builds the route and asks
//! the router helper to install it.
//!
//! Security: nothing here writes a router config or runs nginx. The route
//! goes through the `riku_route` gate in [`build_route`] for an early error
//! and again in the helper, and the helper's answer is data: a refusal is
//! reported with its stage and message, never acted on. Domains reach
//! acme.sh and openssl only after the same validation the route uses.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{anyhow, Result};
use riku_container::types::AppName;
use riku_route::{Reply, Request, RouterKind};

use super::route_build::{build_route, nginx_domains, CATCH_ALL_NAME};
use crate::config::router::Router;
use crate::config::RikuPaths;
use crate::util::backend::Backend;
use crate::util::flag::https_only;

/// The backend a check renders with: the route gate never connects to it.
const STAND_IN_PORT: u16 = 1;

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
    app_path: &Path,
    env: &HashMap<String, String>,
    backends: &[Backend],
    paths: &RikuPaths,
) -> Result<riku_route::Route> {
    match build_route(app, app_path, env, backends, paths, &Router::Nginx) {
        Ok(Some(route)) => Ok(route),
        Ok(None) => Err(anyhow!("'{app}' has no nginx route")),
        Err(e) => Err(e),
    }
}

/// Publish `app`'s static files and certificates, then have the helper
/// install its site and reload nginx. Returns once nginx accepted the reload.
#[allow(clippy::question_mark)]
pub(super) fn apply_nginx_site(
    send: impl Fn(&Request) -> Result<Reply>,
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
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
    if let Err(e) = ensure_certificates(&name, env, paths) {
        return Err(e);
    }
    let route = match nginx_route(&name, app_path, env, backends, paths) {
        Ok(route) => route,
        Err(e) => return Err(e),
    };
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

/// Refuse a deploy before it starts a copy when `app`'s route would fail the
/// gate or the helper is not answering. `nginx -t` itself runs in the helper
/// at apply time.
#[allow(clippy::question_mark)]
pub(super) fn check_nginx_site(
    send: impl Fn(&Request) -> Result<Reply>,
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
) -> Result<()> {
    let name = match app_name(app) {
        Ok(name) => name,
        Err(e) => return Err(e),
    };
    let stand_in = [Backend::loopback(STAND_IN_PORT)];
    if let Err(e) = nginx_route(&name, app_path, env, &stand_in, paths) {
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
