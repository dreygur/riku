//! Router choice during `riku init`: which tree holds the `ROUTER` file,
//! writing `RIKU_ROUTER` into it, and the router specific host setup.
//!
//! A non-root init uses the caller's own riku root. A root init uses the
//! deploy user's, because that is the tree git push deploys read.
//!
//! Security: under root, the `ROUTER` file is read and written only through
//! [`DeployUser`], as the deploy user. Root opening a path in that tree
//! would follow any symlink the deploy user planted there.

use std::env;
use std::fs;
use std::io::ErrorKind;

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

/// The deploy user's router, read as that user. A missing file is nginx,
/// matching [`Router::load`]. `symlink_metadata` does not follow links, so
/// the existence check is safe to make as root.
fn load_as_user(user: &DeployUser, paths: &RikuPaths) -> Result<Router> {
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

#[allow(clippy::question_mark)]
fn save_router(target: &RouterTarget, router: &Router) -> Result<()> {
    let Some(user) = &target.user else {
        return router::save(&target.paths, router);
    };
    if let Err(e) = user.create_dir(&target.paths.riku_root) {
        return Err(e);
    }
    user.write_file(&target.paths.router_file, &format!("{}\n", router.name()))
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
    if let Err(e) = save_router(target, &router) {
        return Err(e);
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
    let loaded = match &user {
        Some(user) => load_as_user(user, &paths),
        None => Router::load(&paths),
    };
    let router = match loaded {
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
