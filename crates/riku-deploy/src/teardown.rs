//! What an app's runtime plugin cleans up when the app is destroyed.

use std::collections::HashMap;

use anyhow::Result;

use crate::config::RikuPaths;
use crate::plugins::runtime::{destroy, detect, discover, RuntimeContext};

/// Ask the runtime plugin that handles `app` to remove what it started
/// outside the app directory, such as compose containers and networks. Runs
/// before the app's files are deleted, since the plugin needs them. Does
/// nothing when the app directory is gone or no plugin claims the app.
#[allow(clippy::question_mark)]
pub fn plugin_destroy(app: &str, paths: &RikuPaths) -> Result<()> {
    let app_path = paths.app_root.join(app);
    if !app_path.exists() {
        return Ok(());
    }
    let env_path = paths.env_root.join(app);
    let mut env = HashMap::new();
    if let Err(e) = crate::util::parse_settings(&env_path.join("ENV"), &mut env) {
        return Err(e);
    }

    let plugins = discover(&paths.plugin_root);
    let plugin = match detect(&plugins, &app_path, &env) {
        Ok(Some(plugin)) => plugin,
        Ok(None) => return Ok(()),
        Err(e) => return Err(e),
    };
    let ctx = RuntimeContext {
        app,
        app_path: &app_path,
        env_path: &env_path,
        riku_root: &paths.riku_root,
        app_env: &env,
    };
    destroy(&plugin, &ctx)
}
