//! Removing an app from the active router.
//!
//! Security: for nginx the router helper removes the site and only the
//! certificate files and sockets in the deploy user's own tree are deleted
//! here, all named from the app name. Nothing under `/etc` is touched by
//! the deploy user.

use anyhow::Result;

use super::nginx_site::remove_nginx_site;
use super::plugin::remove_plugin;
use super::reload::{record_outcome, reload_caddy};
use super::send_to_helper;
use crate::config::router::Router;
use crate::config::RikuPaths;

/// Remove `app` from the active router.
///
/// nginx removal is the router helper's, followed by the deploy user's own
/// leftovers. A removal that finds no site does not reload, and does not
/// touch the recorded reload failure either: no reload was confirmed. For a
/// plugin router this dispatches `unconfigure` (the app's config is keyed by
/// `RIKU_APP`, which is still available even though the app's `ENV` is
/// already gone by destroy time) then `reload`.
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
        Router::Nginx => return remove_nginx_app(app, paths),
        Router::Caddy => match crate::caddy::remove_site(app, paths) {
            Ok(()) => reload_caddy(paths),
            Err(e) => Err(e),
        },
        Router::Plugin(name) => remove_plugin(name, app, paths),
    };
    record_outcome(paths, &outcome);
    outcome
}

fn remove_nginx_app(app: &str, paths: &RikuPaths) -> Result<()> {
    match remove_nginx_site(send_to_helper, app, paths) {
        Ok(true) => {
            record_outcome(paths, &Ok(()));
            Ok(())
        }
        Ok(false) => Ok(()),
        Err(e) => {
            let outcome = Err(e);
            record_outcome(paths, &outcome);
            outcome
        }
    }
}
