//! Single-service image update for compose-based apps.
//!
//! Used by the GHCR webhook to pull and recreate one compose service without
//! running a full [`crate::do_deploy`] — no hooks, no Procfile parsing, no
//! worker respawn, just the one container.

use anyhow::{anyhow, Result};
use std::collections::HashMap;

use crate::config::RikuPaths;
use crate::lock;

/// Pull a fresh image for `service` and recreate just that compose service.
///
/// Serializes against a concurrent [`crate::do_deploy`] of the same app via
/// the existing per-app deploy lock, so a webhook hit can't race a git push.
pub fn pull_service(app: &str, paths: &RikuPaths, service: &str) -> Result<()> {
    let app_path = paths.app_root.join(app);
    if !app_path.exists() {
        return Err(anyhow!(
            "App '{}' not found at {}",
            app,
            app_path.display()
        ));
    }

    let _deploy_lock = lock::acquire(app, paths)?;

    let env_file = paths.env_root.join(app).join("ENV");
    let mut env: HashMap<String, String> = HashMap::new();
    if env_file.exists() {
        crate::util::parse_settings(&env_file, &mut env)?;
    }

    let plugins = crate::plugins::runtime::discover(&paths.plugin_root);
    let plugin = crate::plugins::runtime::detect(&plugins, &app_path, &env)?.ok_or_else(|| {
        anyhow!(
            "No runtime plugin matched '{}'; expected the container plugin for a compose app",
            app
        )
    })?;

    let ctx = crate::plugins::runtime::RuntimeContext {
        app,
        app_path: &app_path,
        env_path: &paths.env_root.join(app),
        riku_root: &paths.riku_root,
        app_env: &env,
    };

    crate::plugins::runtime::pull_service(&plugin, &ctx, service)
}
