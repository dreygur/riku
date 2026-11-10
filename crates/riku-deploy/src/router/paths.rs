//! Publishing the shared path mode site: every app recorded in path mode,
//! each at `/<app>/` on `[routing] shared_port`.
//!
//! The list comes from the routing state, never from resolving every app
//! again, so a broken riku.toml in one app cannot fail another app's deploy.
//! A request goes to the helper only when there is a path app, or when the
//! app being routed just left path mode and its location must go, so a
//! server with no path apps never sends the helper a request it may not
//! know.

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use riku_container::types::AppName;
use riku_route::{Listen, PathApp, Reply, Request, RouterKind};

use super::caddy::remove_caddy_site;
use super::nginx_site::{helper_outcome, remove_nginx_site};
use super::route_build::route_backends;
use super::routing_state::{RoutingMode, RoutingState};
use super::{routed_backends, send_to_helper};
use crate::config::router::Router;
use crate::config::{settings, RikuPaths};
use crate::containers::runs_containers;
use crate::supervisor_ctl::read_app_env;
use crate::util::backend::Backend;

const WSGI_KEY: &str = "RIKU_NGINX_WSGI";

fn router_kind(active: &Router) -> Result<RouterKind> {
    match active {
        Router::Nginx => Ok(RouterKind::Nginx),
        Router::Caddy => Ok(RouterKind::Caddy),
        Router::Plugin(name) => Err(anyhow!("router plugin '{name}' does not do path mode")),
    }
}

#[allow(clippy::question_mark)]
fn path_app(app: &str, backends: Vec<Backend>, paths: &RikuPaths) -> Result<PathApp> {
    let name = match AppName::try_from(app) {
        Ok(name) => name,
        Err(e) => return Err(anyhow!("cannot route '{app}' by path: {e}")),
    };
    match route_backends(&name, &backends, paths) {
        Ok(backends) => Ok(PathApp {
            app: app.to_string(),
            backends,
        }),
        Err(e) => Err(anyhow!("cannot route '{app}' by path: {e}")),
    }
}

/// [`publish_paths`] with the helper and the backend lookup for the other
/// path apps given. `current` is the app being routed and its backends;
/// `leaving` is true when it just left path mode.
#[allow(clippy::question_mark)]
pub(super) fn publish_paths_with(
    send: impl Fn(&Request) -> Result<Reply>,
    lookup: impl Fn(&str) -> Result<Vec<Backend>>,
    paths: &RikuPaths,
    active: &Router,
    current: (&str, Vec<Backend>),
    leaving: bool,
) -> Result<()> {
    let state = match RoutingState::load(paths) {
        Ok(state) => state,
        Err(e) => return Err(e),
    };
    let path_apps: Vec<&str> = state
        .entries()
        .filter(|(_, entry)| entry.mode == RoutingMode::Path)
        .map(|(app, _)| app)
        .collect();
    if path_apps.is_empty() && !leaving {
        return Ok(());
    }
    let router = match router_kind(active) {
        Ok(router) => router,
        Err(e) => return Err(e),
    };
    let shared_port = match settings::load(paths) {
        Ok(loaded) => loaded.routing.shared_port,
        Err(e) => return Err(e),
    };
    let (current_app, current_backends) = current;
    let mut apps = Vec::with_capacity(path_apps.len());
    for app in path_apps {
        let backends = if app == current_app {
            current_backends.clone()
        } else {
            match lookup(app) {
                Ok(backends) => backends,
                Err(e) => return Err(e),
            }
        };
        match path_app(app, backends, paths) {
            Ok(entry) => apps.push(entry),
            Err(e) => return Err(e),
        }
    }
    let request = Request::ApplyPaths {
        router,
        listen: Listen { port: shared_port },
        apps,
    };
    match send(&request) {
        Ok(reply) => helper_outcome(reply, "install the path site").map(|_| ()),
        Err(e) => Err(e),
    }
}

/// Send the helper the path site for every path mode app. Other apps'
/// backends are read from their ENV and running copies.
pub(crate) fn publish_paths(
    paths: &RikuPaths,
    active: &Router,
    current: (&str, Vec<Backend>),
    leaving: bool,
) -> Result<()> {
    let lookup = |app: &str| match read_app_env(app, paths) {
        Ok(env) => routed_backends(active, app, &env, paths, None),
        Err(e) => Err(e),
    };
    publish_paths_with(
        |request: &Request| send_to_helper(paths, request),
        lookup,
        paths,
        active,
        current,
        leaving,
    )
}

/// The mode `app` had before this apply, from the routing state.
pub(crate) fn previous_mode(app: &str, paths: &RikuPaths) -> Option<RoutingMode> {
    match RoutingState::load(paths) {
        Ok(state) => state.entry(app).map(|entry| entry.mode),
        Err(_) => None,
    }
}

/// Route a path mode app: drop its own site and publish the path site with
/// it in. A uWSGI app speaks uwsgi, not HTTP, and a container app's copies
/// come from the engine, so neither takes path mode yet.
#[allow(clippy::question_mark)]
pub(super) fn route_by_path(
    active: &Router,
    app: &str,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
    backends: &[Backend],
) -> Result<()> {
    if env.contains_key(WSGI_KEY) {
        return Err(anyhow!(
            "'{app}' is a uWSGI app; path mode proxies HTTP only"
        ));
    }
    match runs_containers(app, paths) {
        Ok(true) => {
            return Err(anyhow!(
                "'{app}' runs containers; path mode is for Procfile apps"
            ))
        }
        Ok(false) => {}
        Err(e) => return Err(e),
    }
    let send = |request: &Request| send_to_helper(paths, request);
    let removed = match active {
        Router::Nginx => remove_nginx_site(send, app, paths),
        Router::Caddy => remove_caddy_site(send, app),
        Router::Plugin(name) => {
            return Err(anyhow!("router plugin '{name}' does not do path mode"))
        }
    };
    if let Err(e) = removed {
        return Err(e);
    }
    publish_paths(paths, active, (app, backends.to_vec()), false)
}

#[cfg(test)]
#[path = "paths_tests.rs"]
mod tests;
