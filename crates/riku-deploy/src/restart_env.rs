//! The env `riku restart` gives a Procfile app's workers: what a deploy
//! computes for them, the merged ENV and riku settings followed by the
//! runtime plugin's `env` output, without a build, release or preflight and
//! without touching the checkout.
//!
//! The plugin's output is the one the last deploy stored (see
//! `plugin_env.rs`), since a plugin's `env` may depend on what its build
//! left. An app last deployed by a riku that stored none falls back to
//! running the plugin's `env`, and says so.

// Project rule 16 forbids `?`, and this lint asks for exactly that rewrite.
#![allow(clippy::question_mark)]

use std::collections::HashMap;

use anyhow::Result;

use crate::config::RikuPaths;
use crate::plugins::runtime::{get_env, RuntimeContext};
use crate::util::display;
use crate::{deploy_env, plugin_env, procfile_deploy};

/// Runs the detected runtime plugin's `env` subcommand, never `build`.
fn run_plugin_env(
    app: &str,
    paths: &RikuPaths,
    env: &HashMap<String, String>,
) -> Result<HashMap<String, String>> {
    let app_path = paths.app_root.join(app);
    let plugin = match procfile_deploy::detect_runtime(app, &app_path, paths, env) {
        Ok(plugin) => plugin,
        Err(e) => return Err(e),
    };
    let env_path = paths.env_root.join(app);
    let ctx = RuntimeContext {
        app,
        app_path: &app_path,
        env_path: &env_path,
        riku_root: &paths.riku_root,
        app_env: env,
    };
    get_env(&plugin, &ctx)
}

/// The worker env of `app` from its current config and the plugin env the
/// last deploy stored.
pub(crate) fn worker_env(app: &str, paths: &RikuPaths) -> Result<HashMap<String, String>> {
    let mut env = match deploy_env::load_restart_env(app, paths) {
        Ok(env) => env,
        Err(e) => return Err(e),
    };
    let plugin_env = match plugin_env::load(app, paths) {
        Ok(Some(stored)) => stored,
        Ok(None) => {
            display::detail(
                "no runtime plugin env stored by the last deploy; running the plugin's env \
                 (the next deploy stores it)",
            );
            match run_plugin_env(app, paths, &env) {
                Ok(fresh) => fresh,
                Err(e) => return Err(e),
            }
        }
        Err(e) => return Err(e),
    };
    env.extend(plugin_env);
    Ok(env)
}

#[cfg(test)]
#[path = "restart_env_tests.rs"]
mod tests;
