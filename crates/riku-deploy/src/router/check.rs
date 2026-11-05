//! Checking an app's router config before a deploy starts a copy.

use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;

use super::nginx_site::check_nginx_site;
use super::send_to_helper;
use crate::config::router::Router;
use crate::config::RikuPaths;

/// Build `app`'s nginx route and ask the helper whether it answers, leaving
/// the live config alone. A deterministic failure, such as a refused
/// `RIKU_NGINX_INCLUDE_FILE` snippet, or a helper that is not running, then
/// fails the deploy before any copy starts. `nginx -t` runs in the helper
/// when the site is applied. Caddy and router plugins have nothing to check
/// here.
#[allow(clippy::question_mark)]
pub fn check(
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
) -> Result<()> {
    let active = match Router::load(paths) {
        Ok(router) => router,
        Err(e) => return Err(e),
    };
    match active {
        Router::Nginx => check_nginx_site(send_to_helper, app, app_path, env, paths),
        Router::Caddy | Router::Plugin(_) => Ok(()),
    }
}
