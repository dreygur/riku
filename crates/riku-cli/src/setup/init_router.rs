//! Router during `riku init`: storing the chosen router in `settings.toml`
//! as `[router] name`, reading it back, and the router specific host setup.
//!
//! The file is always written by the user whose tree it lives in: a plain
//! init writes its own, and a root init hands that step to the deploy user.
//!
//! Security: root reads the deploy user's `settings.toml` only through
//! [`DeployUser`], as that user. Root opening a path in
//! that tree would follow any symlink the deploy user planted there.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use anyhow::{anyhow, Result};

use crate::config::router::{self, Router};
use crate::config::{settings, RikuPaths};
use crate::util::display;

use super::caddy_host::setup_caddy_host;
use super::deploy_user::DeployUser;
use super::nginx_host::setup_nginx_host;

/// Store `router` in `paths`' settings.toml. The choice itself is made by
/// [`super::router_choice`].
#[allow(clippy::question_mark)]
pub fn persist_router(paths: &RikuPaths, router: &Router) -> Result<()> {
    if let Err(e) = fs::create_dir_all(&paths.riku_root) {
        return Err(anyhow!(
            "failed to create {}: {e}",
            paths.riku_root.display()
        ));
    }
    if let Err(e) = router::save(paths, router) {
        return Err(e);
    }
    display::step_done("Router", router.name());
    Ok(())
}

/// `file` in the deploy user's tree, read as that user; `None` when missing.
/// `symlink_metadata` does not follow links, so the existence check is safe
/// to make as root.
fn read_as_deploy_user(user: &DeployUser, file: &Path) -> Result<Option<String>> {
    match fs::symlink_metadata(file) {
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(anyhow!("failed to stat {}: {e}", file.display())),
        Ok(_) => {}
    }
    match user.read_file(file) {
        Ok(text) => Ok(Some(text)),
        Err(e) => Err(e),
    }
}

/// The deploy user's settings.toml, read as that user; `None` when missing.
pub(super) fn deploy_settings(user: &DeployUser) -> Result<Option<settings::ServerSettings>> {
    let paths = user.riku_paths();
    let raw = match read_as_deploy_user(user, &paths.settings_file) {
        Ok(Some(raw)) => raw,
        Ok(None) => return Ok(None),
        Err(e) => return Err(e),
    };
    match settings::parse(&raw, &paths.settings_file) {
        Ok(parsed) => Ok(Some(parsed)),
        Err(e) => Err(e),
    }
}

/// The deploy user's stored router from settings.toml, read as that user, as
/// [`router::stored`] reads it; `None` when it is not set.
pub fn deploy_router(user: &DeployUser) -> Result<Option<Router>> {
    match deploy_settings(user) {
        Ok(parsed) => Ok(parsed
            .and_then(|parsed| parsed.router)
            .map(|name| Router::from(name.as_str()))),
        Err(e) => Err(e),
    }
}

/// Router specific host setup that needs root. A router plugin has none.
pub fn setup_router_host(router: &Router, user: &DeployUser) -> Result<()> {
    match router {
        Router::Caddy => setup_caddy_host(&user.riku_paths()),
        Router::Nginx => setup_nginx_host(user),
        Router::Plugin(_) => Ok(()),
    }
}
