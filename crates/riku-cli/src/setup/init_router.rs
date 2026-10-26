//! Router choice during `riku init`: which tree holds the `ROUTER` file,
//! writing `RIKU_ROUTER` into it, and the router specific host setup.
//!
//! A non-root init uses the caller's own riku root. A root init uses the
//! deploy user's, because that is the tree git push deploys read.

use std::env;
use std::fs;

use anyhow::{anyhow, Result};

use crate::config::router::{self, Router};
use crate::config::RikuPaths;
use crate::util::echo;

use super::caddy_host::setup_caddy_host;
use super::deploy_user::DeployUser;

/// Where the host router lives and which one it is.
pub struct RouterTarget {
    /// The riku paths deploys read: the deploy user's under root, else ours.
    pub paths: RikuPaths,
    /// The deploy user, looked up only under root.
    pub user: Option<DeployUser>,
    pub router: Router,
}

#[allow(clippy::question_mark)]
fn ensure_riku_root(target: &RouterTarget) -> Result<()> {
    let root = &target.paths.riku_root;
    if root.exists() {
        return Ok(());
    }
    if let Err(e) = fs::create_dir_all(root) {
        return Err(anyhow!("failed to create {}: {e}", root.display()));
    }
    match &target.user {
        Some(user) => user.chown(root),
        None => Ok(()),
    }
}

/// Write `RIKU_ROUTER` to the target's `ROUTER` file. Init is the only
/// reader of the env var; unset leaves an existing choice alone.
#[allow(clippy::question_mark)]
fn persist_router(target: &mut RouterTarget) -> Result<()> {
    let raw = env::var("RIKU_ROUTER").unwrap_or_default();
    if raw.trim().is_empty() {
        return Ok(());
    }

    let router = Router::from(raw.as_str());
    if let Err(e) = ensure_riku_root(target) {
        return Err(e);
    }
    if let Err(e) = router::save(&target.paths, &router) {
        return Err(e);
    }
    if let Some(user) = &target.user {
        if let Err(e) = user.chown(&target.paths.router_file) {
            return Err(e);
        }
    }
    echo(&format!("      ✓ router: {}", router.name()), "green");
    target.router = router;
    Ok(())
}

/// Resolve the router target and apply `RIKU_ROUTER` if set.
#[allow(clippy::question_mark)]
pub fn resolve_router(own_paths: &RikuPaths, is_root: bool) -> Result<RouterTarget> {
    let user = if is_root {
        match DeployUser::lookup() {
            Ok(user) => Some(user),
            Err(e) => return Err(e),
        }
    } else {
        None
    };
    let paths = match &user {
        Some(user) => user.riku_paths(),
        None => own_paths.clone(),
    };
    let router = match Router::load(&paths) {
        Ok(router) => router,
        Err(e) => return Err(e),
    };

    let mut target = RouterTarget {
        paths,
        user,
        router,
    };
    if let Err(e) = persist_router(&mut target) {
        return Err(e);
    }
    Ok(target)
}

/// Router specific host setup that needs root. Only Caddy has any.
pub fn setup_router_host(target: &RouterTarget) -> Result<()> {
    match (&target.router, &target.user) {
        (Router::Caddy, Some(user)) => setup_caddy_host(&target.paths, user),
        _ => Ok(()),
    }
}
