//! What `riku doctor --fix` changes before the checks run: redeploying
//! apps missing from the router's site root, and rewriting stale launch
//! paths.

use std::collections::HashMap;
use std::path::Path;

use crate::config::router::Router;
use crate::config::{RikuPaths, CADDY_SITE_ROOT, NGINX_SITE_ROOT};

use super::{redeploy_sites, stale_paths, Check};

/// Redeploy the apps that have no site file in the active router's site
/// root. A deploy runs the app's build, which is the deploy user's code, so it
/// is refused as root. A plugin router keeps its own sites, so nothing is
/// redeployed for it.
pub(super) fn redeploy_old_sites(paths: &RikuPaths) -> Vec<Check> {
    if nix::unistd::Uid::effective().is_root() {
        return vec![Check::warn(
            "router sites",
            "redeploy skipped as root: run `riku doctor --fix` as the deploy user",
        )];
    }
    let (site_root, extension) = match Router::load(paths) {
        Ok(Router::Nginx) => (NGINX_SITE_ROOT, "conf"),
        Ok(Router::Caddy) => (CADDY_SITE_ROOT, "caddy"),
        Ok(Router::Plugin(_)) => return Vec::new(),
        Err(e) => return vec![Check::fail("router sites", e.to_string())],
    };
    redeploy_sites::redeploy_unrouted_sites(
        &paths.riku_root,
        Path::new(site_root),
        extension,
        // An app whose routing cannot be read is redeployed, as before;
        // the deploy then reports why.
        |app| crate::deploy::router::site_expected(app, paths).unwrap_or(true),
        |app| crate::deploy::do_deploy(app, paths, &HashMap::new(), None),
    )
}

/// Rewrite stale forced commands and post-receive hooks. They are the deploy
/// user's own files, so this is refused as root, like the redeploy step.
pub(super) fn fix_launch_paths(paths: &RikuPaths) -> Vec<Check> {
    if nix::unistd::Uid::effective().is_root() {
        return vec![Check::warn(
            "launch paths",
            "rewrite skipped as root: run `riku doctor --fix` as the deploy user",
        )];
    }
    let home = match std::env::var("HOME") {
        Ok(home) => home,
        Err(_) => {
            return vec![Check::warn(
                "launch paths",
                "HOME not set, nothing rewritten",
            )]
        }
    };
    let keys = crate::util::authorized_keys::authorized_keys_path(Path::new(&home));
    stale_paths::fix_stale_launch_paths(&keys, &paths.git_root)
}
