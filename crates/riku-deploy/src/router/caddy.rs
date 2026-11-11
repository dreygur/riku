//! Caddy branch of the router seam: turns an app's ENV into a route and asks
//! the router helper to install or clear its site.
//!
//! Security: nothing here writes a site file or runs Caddy. Domains go through
//! `build_route`, so a value like `a.com {` fails the deploy, and the
//! route passes the `riku_route` Caddy gate here and again in the helper. The
//! helper's answer is data: a refusal is reported with its stage, never
//! acted on.

use std::collections::HashMap;

use anyhow::Result;
use riku_route::{Reply, Request, RouterKind};

use super::nginx_site::{app_name, helper_outcome};
use super::route_build::build_route;
use crate::config::router::Router;
use crate::config::RikuPaths;
use crate::util::backend::Backend;

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
    let route = match build_route(&name, env, backends, paths, &Router::Caddy) {
        Ok(Some(route)) => route,
        Ok(None) => return remove_caddy_site(send, app).map(|_| ()),
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
