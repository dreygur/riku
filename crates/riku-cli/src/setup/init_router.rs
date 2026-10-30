//! Router choice during `riku init`: writing `RIKU_ROUTER` into the `ROUTER`
//! file, reading it back, and the router specific host setup.
//!
//! The file is always written by the user whose tree it lives in: a plain
//! init writes its own, and a root init hands that step to the deploy user.
//!
//! Security: root reads the deploy user's `ROUTER` file only through
//! [`DeployUser`], as that user. Root opening a path in that tree would
//! follow any symlink the deploy user planted there.

use std::env;
use std::fs;
use std::io::ErrorKind;

use anyhow::{anyhow, Result};

use crate::config::router::{self, Router};
use crate::config::RikuPaths;
use crate::util::echo;

use super::caddy_host::setup_caddy_host;
use super::deploy_user::DeployUser;

/// Write `RIKU_ROUTER` to `paths`' `ROUTER` file and return the router now in
/// effect. Init is the only reader of the env var; unset leaves an existing
/// choice alone.
#[allow(clippy::question_mark)]
pub fn persist_router(paths: &RikuPaths) -> Result<Router> {
    let raw = env::var("RIKU_ROUTER").unwrap_or_default();
    if raw.trim().is_empty() {
        return Router::load(paths);
    }

    let router = Router::from(raw.as_str());
    if let Err(e) = fs::create_dir_all(&paths.riku_root) {
        return Err(anyhow!(
            "failed to create {}: {e}",
            paths.riku_root.display()
        ));
    }
    if let Err(e) = router::save(paths, &router) {
        return Err(e);
    }
    echo(&format!("      ✓ router: {}", router.name()), "green");
    Ok(router)
}

/// The deploy user's router, read as that user. A missing file is nginx,
/// matching [`Router::load`]. `symlink_metadata` does not follow links, so
/// the existence check is safe to make as root.
pub fn deploy_router(user: &DeployUser) -> Result<Router> {
    let paths = user.riku_paths();
    match fs::symlink_metadata(&paths.router_file) {
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Router::Nginx),
        Err(e) => {
            return Err(anyhow!(
                "failed to stat {}: {e}",
                paths.router_file.display()
            ))
        }
        Ok(_) => {}
    }
    match user.read_file(&paths.router_file) {
        Ok(name) => Ok(Router::from(name.as_str())),
        Err(e) => Err(e),
    }
}

/// Router specific host setup that needs root. Only Caddy has any.
pub fn setup_router_host(router: &Router, user: &DeployUser) -> Result<()> {
    match router {
        Router::Caddy => setup_caddy_host(&user.riku_paths(), user),
        Router::Nginx | Router::Plugin(_) => Ok(()),
    }
}
