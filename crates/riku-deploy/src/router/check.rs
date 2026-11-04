//! Checking an app's router config before a deploy starts a copy.

use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;

use crate::config::router::Router;
use crate::config::RikuPaths;
use crate::util::backend::Backend;

/// The backend the check renders with. The backend list only changes the
/// upstream's `server` lines, and `nginx -t` never connects to them.
const STAND_IN_PORT: u16 = 1;

/// Render `app`'s nginx config and have `nginx -t` check it, leaving the live
/// config alone. A deterministic failure, such as a refused
/// `RIKU_NGINX_INCLUDE_FILE` snippet, then fails the deploy before any copy
/// starts. Caddy and router plugins have nothing to check here.
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
        Router::Nginx => crate::nginx::check_nginx_config(
            app,
            app_path,
            env,
            &[Backend::loopback(STAND_IN_PORT)],
            paths,
        ),
        Router::Caddy | Router::Plugin(_) => Ok(()),
    }
}
