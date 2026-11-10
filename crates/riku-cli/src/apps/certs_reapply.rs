//! Putting a changed certificate pair live without a deploy: the route is
//! rebuilt from the app's saved ENV and running copies, as a certificate
//! renewal does, and sent to the active router.

#![allow(clippy::question_mark)]

use anyhow::{anyhow, Result};

use crate::config::router::Router;
use crate::config::RikuPaths;
use crate::deploy::containers::{open_engine, runs_containers};

/// Apply `app`'s current route again. nginx goes through the same path as
/// `riku router-renew`; any other router gets a full `configure`.
pub(super) fn reapply_route(paths: &RikuPaths, app: &str) -> Result<()> {
    let active = match Router::load(paths) {
        Ok(router) => router,
        Err(e) => return Err(e),
    };
    if matches!(active, Router::Nginx) {
        return crate::deploy::router::renew(app, paths);
    }
    reconfigure(paths, app)
}

/// A full `configure` of `app`'s route, as a deploy runs it: with no pair on
/// disk it issues a certificate (nginx) or lets Caddy issue its own.
pub(super) fn reconfigure(paths: &RikuPaths, app: &str) -> Result<()> {
    let env = match crate::config::app_config::load_env(paths, app) {
        Ok(loaded) => loaded.env,
        Err(e) => return Err(anyhow!("cannot read the config of '{app}': {e:#}")),
    };
    let engine = match runs_containers(app, paths) {
        Ok(true) => match open_engine(paths) {
            Ok(engine) => Some(engine),
            Err(e) => return Err(e),
        },
        Ok(false) => None,
        Err(e) => return Err(e),
    };
    crate::deploy::router::configure(app, &paths.app_root.join(app), &env, paths, engine.as_ref())
}
