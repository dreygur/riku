//! Removing an app from the active router.
//!
//! Security: for nginx only paths built from the app name inside
//! `nginx_root` are touched, never anything under `/etc/nginx`, and the
//! reload uses the same fixed programs as a deploy.

use anyhow::Result;

use super::plugin::remove_plugin;
use super::reload::{record_outcome, reload, Baseline};
use crate::config::router::Router;
use crate::config::RikuPaths;

/// Remove `app` from the active router.
///
/// nginx removal is built-in. For a plugin router this dispatches `unconfigure`
/// (the app's config is keyed by `RIKU_APP`, which is still available even
/// though the app's `ENV` is already gone by destroy time) then `reload`.
///
/// `unconfigure` is **best-effort**: it was added within API v1, so a plugin
/// predating it may not implement the verb and will exit non-zero, in that
/// case the following `reload` lets the plugin reconcile the orphaned upstream
/// on its own. See `PLUGIN_PROTOCOL.md` §6.2.
#[allow(clippy::question_mark)]
pub fn remove(app: &str, paths: &RikuPaths) -> Result<()> {
    let active = match Router::load(paths) {
        Ok(router) => router,
        Err(e) => return Err(e),
    };

    let outcome = match &active {
        Router::Nginx => remove_nginx_app(app, paths, &active),
        Router::Caddy => match crate::caddy::remove_site(app, paths) {
            Ok(()) => reload(&active, paths, &Baseline::Direct),
            Err(e) => Err(e),
        },
        Router::Plugin(name) => remove_plugin(name, app, paths),
    };
    record_outcome(paths, &outcome);
    outcome
}

/// Delete `app`'s nginx files, then reload nginx the way a deploy does if
/// its config was live. The reload method is decided before the files go,
/// so a reload unit watching the config directory sees the change. When a
/// side file could not be removed the reload still runs, then that error
/// is returned.
#[allow(clippy::question_mark)]
fn remove_nginx_app(app: &str, paths: &RikuPaths, active: &Router) -> Result<()> {
    let baseline = Baseline::capture(&paths.reload_programs);
    let removal = match crate::nginx::remove_nginx_files(app, paths) {
        Ok(removal) => removal,
        Err(e) => return Err(e),
    };
    let reloaded = if removal.was_live {
        reload(active, paths, &baseline)
    } else {
        Ok(())
    };
    match (reloaded, removal.leftover_error) {
        (Err(e), Some(leftover)) => {
            tracing::warn!("also could not remove a file of '{app}': {leftover}");
            Err(e)
        }
        (Err(e), None) => Err(e),
        (Ok(()), Some(leftover)) => Err(leftover),
        (Ok(()), None) => Ok(()),
    }
}

#[cfg(test)]
#[path = "remove_tests.rs"]
mod tests;
