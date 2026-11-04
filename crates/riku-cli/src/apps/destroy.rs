//! `riku destroy`: stop an app's containers and runtime, remove its route,
//! then its files (see [`super::destroy_files`]). Data and cache stay.

use anyhow::Result;

use super::destroy_engine::container_cleanup;
use super::destroy_files::remove_app_files;
use crate::config::RikuPaths;
use crate::util::{echo, exit_if_invalid};

/// The runtime plugin removes what it started outside the app directory.
/// A failure is reported and destroy goes on: the app is going away either
/// way, and its files are what the plugin would need to retry.
fn plugin_cleanup(paths: &RikuPaths, app: &str) {
    if let Err(e) = crate::deploy::teardown::plugin_destroy(app, paths) {
        echo(
            &format!("--> Runtime cleanup failed, containers may remain: {e}"),
            "yellow",
        );
    }
}

/// What destroy does for a container app whose engine cannot be reached.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DestroyMode {
    /// Refuse and remove nothing, so no container is orphaned.
    Safe,
    /// Remove riku's files and route anyway and leave the containers running.
    Force,
}

impl From<bool> for DestroyMode {
    fn from(force: bool) -> Self {
        if force {
            DestroyMode::Force
        } else {
            DestroyMode::Safe
        }
    }
}

/// Destroy an app: remove directories and config files, preserve data/cache.
#[allow(clippy::question_mark)]
pub fn cmd_destroy(paths: &RikuPaths, app: &str, mode: DestroyMode) -> Result<()> {
    let app = match exit_if_invalid(app, &paths.app_root) {
        Ok(app) => app,
        Err(e) => return Err(e),
    };

    if let Err(e) = container_cleanup(paths, &app, mode) {
        if e.downcast_ref::<crate::deploy::containers::EngineUnreachable>()
            .is_some()
        {
            return Err(e);
        }
        return Err(e.context(format!(
            "cannot remove the containers of '{app}', nothing was deleted"
        )));
    }
    plugin_cleanup(paths, &app);

    // First, while the app still exists: a failure here leaves an app a
    // retried destroy can find, not a router config with no app.
    echo(
        &format!("--> Removing router config for '{}'", app),
        "yellow",
    );
    if let Err(e) = crate::deploy::router::remove(&app, paths) {
        return Err(e);
    }
    if let Err(e) = remove_app_files(paths, &app) {
        return Err(e);
    }

    for dir in [&paths.data_root, &paths.cache_root] {
        let path = dir.join(&app);
        if path.exists() {
            echo(
                &format!("==> Preserving folder '{}'", path.display()),
                "red",
            );
        }
    }

    Ok(())
}

#[cfg(test)]
#[path = "destroy_tests.rs"]
mod tests;
