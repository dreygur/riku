//! Single service image update for container and legacy compose apps.
//!
//! Called by the supervisor's periodic image watch. For riku container apps
//! it pulls the service image, compares engine image ids and replaces only
//! that service's copies when they differ. Legacy compose apps still go
//! through the container plugin's `pull_service` until they are migrated.

// Project rule 16 forbids `?`, and this lint asks for exactly that rewrite.
#![allow(clippy::question_mark)]

use std::collections::HashMap;

use anyhow::{anyhow, Result};

use crate::config::RikuPaths;
use crate::containers::{is_built_service, AppName, ImageUpdate, ServiceName};
use crate::lock;

fn container_pull(app: &str, paths: &RikuPaths, service: &str) -> Result<()> {
    let app_name = match AppName::try_from(app) {
        Ok(name) => name,
        Err(e) => return Err(e),
    };
    let service_name = match ServiceName::try_from(service) {
        Ok(name) => name,
        Err(e) => return Err(e),
    };
    let is_built = match is_built_service(&app_name, paths, &service_name) {
        Ok(is_built) => is_built,
        Err(e) => return Err(e),
    };
    if is_built {
        tracing::info!(app, service, outcome = ?ImageUpdate::Built, "image watch check");
        return Ok(());
    }
    let update = match crate::containers::update_service(&app_name, paths, &service_name) {
        Ok(update) => update,
        Err(e) => return Err(e),
    };
    tracing::info!(app, service, outcome = ?update, "image watch check");
    Ok(())
}

fn plugin_pull(app: &str, paths: &RikuPaths, service: &str) -> Result<()> {
    let app_path = paths.app_root.join(app);
    if !app_path.exists() {
        return Err(anyhow!("App '{}' not found at {}", app, app_path.display()));
    }

    let _deploy_lock = match lock::acquire(app, paths) {
        Ok(lock) => lock,
        Err(e) => return Err(e),
    };

    let env_file = paths.env_root.join(app).join("ENV");
    let mut env: HashMap<String, String> = HashMap::new();
    if env_file.exists() {
        if let Err(e) = crate::util::parse_settings(&env_file, &mut env) {
            return Err(e);
        }
    }

    let plugins = crate::plugins::runtime::discover(&paths.plugin_root);
    let detected = match crate::plugins::runtime::detect(&plugins, &app_path, &env) {
        Ok(detected) => detected,
        Err(e) => return Err(e),
    };
    let plugin = match detected {
        Some(plugin) => plugin,
        None => {
            return Err(anyhow!(
                "No runtime plugin matched '{}'; expected the container plugin for a compose app",
                app
            ))
        }
    };

    let ctx = crate::plugins::runtime::RuntimeContext {
        app,
        app_path: &app_path,
        env_path: &paths.env_root.join(app),
        riku_root: &paths.riku_root,
        app_env: &env,
    };

    crate::plugins::runtime::pull_service(&plugin, &ctx, service)
}

/// Pull a fresh image for `service` and update that service.
///
/// Serializes against a concurrent [`crate::do_deploy`] of the same app via
/// the existing per app deploy lock, so a periodic check cannot race a git push.
pub fn pull_service(app: &str, paths: &RikuPaths, service: &str) -> Result<()> {
    match crate::containers::runs_containers(app, paths) {
        Ok(true) => container_pull(app, paths, service),
        Ok(false) => plugin_pull(app, paths, service),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
#[path = "service_update_tests.rs"]
mod tests;
