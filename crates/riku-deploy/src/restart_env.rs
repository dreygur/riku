//! The env `riku restart` gives a Procfile app's workers: what a deploy
//! computes for them, the merged ENV and riku settings followed by the
//! runtime plugin's `env` output, without a build, release or preflight and
//! without touching the checkout.

// Project rule 16 forbids `?`, and this lint asks for exactly that rewrite.
#![allow(clippy::question_mark)]

use std::collections::HashMap;

use anyhow::Result;

use crate::config::RikuPaths;
use crate::plugins::runtime::{get_env, RuntimeContext};
use crate::{deploy_env, procfile_deploy};

/// The worker env of `app` from its current config. Runs the detected
/// runtime plugin's `env` subcommand, never `build`.
pub(crate) fn worker_env(app: &str, paths: &RikuPaths) -> Result<HashMap<String, String>> {
    let mut env = match deploy_env::load_restart_env(app, paths) {
        Ok(env) => env,
        Err(e) => return Err(e),
    };
    let app_path = paths.app_root.join(app);
    let plugin = match procfile_deploy::detect_runtime(app, &app_path, paths, &env) {
        Ok(plugin) => plugin,
        Err(e) => return Err(e),
    };
    let env_path = paths.env_root.join(app);
    let ctx = RuntimeContext {
        app,
        app_path: &app_path,
        env_path: &env_path,
        riku_root: &paths.riku_root,
        app_env: &env,
    };
    let plugin_env = match get_env(&plugin, &ctx) {
        Ok(plugin_env) => plugin_env,
        Err(e) => return Err(e),
    };
    env.extend(plugin_env);
    Ok(env)
}
