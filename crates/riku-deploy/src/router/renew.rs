//! `riku router-renew <app>`: send the app's current certificate files to the
//! router helper after acme.sh renewed them.
//!
//! The helper keeps its own copy under `/etc/riku/certs/<app>`, so a renewal
//! in the deploy user's tree changes nothing until a new route is applied.
//! acme.sh calls this as its `--reloadcmd`.
//!
//! Security: the app name is validated before anything is read from disk, and
//! the route is built and sent exactly as a deploy does, so the helper gates
//! it again. Nothing here takes a path or a certificate from the caller.

use anyhow::{anyhow, Result};
use riku_route::{Reply, Request};

use super::nginx_site::{app_name, apply_nginx_site};
use super::reload::record_outcome;
use super::routed_backends;
use super::send_to_helper;
use crate::config::router::Router;
use crate::config::RikuPaths;
use crate::containers::{open_engine, runs_containers};
use crate::supervisor_ctl::read_app_env;

/// Rebuild `app`'s nginx route from its saved ENV and running copies, then
/// have the helper install it with the files now in `nginx_root`.
pub fn renew(app: &str, paths: &RikuPaths) -> Result<()> {
    renew_with(send_to_helper, app, paths)
}

#[allow(clippy::question_mark)]
fn renew_with(
    send: impl Fn(&Request) -> Result<Reply>,
    app: &str,
    paths: &RikuPaths,
) -> Result<()> {
    let name = match app_name(app) {
        Ok(name) => name,
        Err(e) => return Err(e),
    };
    let app_path = paths.app_root.join(name.as_str());
    if !app_path.is_dir() {
        return Err(anyhow!("no app named '{app}'"));
    }
    let active = match Router::load(paths) {
        Ok(router) => router,
        Err(e) => return Err(e),
    };
    if !matches!(active, Router::Nginx) {
        return Err(anyhow!(
            "certificates are installed through the router helper only for nginx"
        ));
    }
    let env = match read_app_env(name.as_str(), paths) {
        Ok(env) => env,
        Err(e) => return Err(e),
    };
    let engine = match runs_containers(name.as_str(), paths) {
        Ok(true) => match open_engine(paths) {
            Ok(engine) => Some(engine),
            Err(e) => return Err(e),
        },
        Ok(false) => None,
        Err(e) => return Err(e),
    };
    let backends = match routed_backends(&active, name.as_str(), &env, paths, engine.as_ref()) {
        Ok(backends) => backends,
        Err(e) => return Err(e),
    };
    let outcome = apply_nginx_site(send, name.as_str(), &app_path, &env, &backends, paths);
    record_outcome(paths, &outcome);
    outcome
}

#[cfg(test)]
#[path = "renew_tests.rs"]
mod tests;
