//! Checking an app's router config before a deploy starts a copy.

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use riku_route::Request;

use super::nginx_site::check_nginx_site;
use super::route_build::refuse_nginx_only_settings;
use super::routing_env::{routing_env, Record};
use super::send_to_helper;
use crate::config::router::Router;
use crate::config::RikuPaths;

/// Build `app`'s nginx route and ask the helper whether it answers, leaving
/// the live config alone. A deterministic failure, such as an ENV key the
/// core route dropped, or a helper that is not running, then
/// fails the deploy before any copy starts. `nginx -t` runs in the helper
/// when the site is applied. For Caddy only a setting it would drop, such
/// as `routing.static_paths`, is refused; router plugins have nothing to
/// check here.
#[allow(clippy::question_mark)]
pub fn check(app: &str, env: &HashMap<String, String>, paths: &RikuPaths) -> Result<()> {
    let active = match Router::load(paths) {
        Ok(router) => router,
        Err(e) => return Err(e),
    };
    let routed = match routing_env(app, env, paths, &active, Record::Peek) {
        Ok((routed, _)) => routed,
        Err(e) => return Err(anyhow!("cannot route '{app}': {e}")),
    };
    match active {
        Router::Nginx => {
            let send = |request: &Request| send_to_helper(paths, request);
            check_nginx_site(send, app, &routed, paths)
        }
        Router::Caddy => match refuse_nginx_only_settings(&routed) {
            Ok(()) => Ok(()),
            Err(e) => Err(anyhow!("cannot route '{app}': {e}")),
        },
        Router::Plugin(_) => Ok(()),
    }
}
