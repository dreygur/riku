//! The env `riku restart` gives a Procfile app's workers: what a deploy
//! computes for them, the merged ENV and riku settings followed by the
//! runtime plugin's `env` output, without a build, release or preflight and
//! without touching the checkout.
//!
//! The plugin's output is the one the last deploy stored (see
//! `plugin_env.rs`), since a plugin's `env` may depend on what its build
//! left. When the app's ENV or server settings changed since it was stored,
//! or none was stored, the plugin's `env` runs again with the current env,
//! restart says so, and its output is stored for the next restart.

// Project rule 16 forbids `?`, and this lint asks for exactly that rewrite.
#![allow(clippy::question_mark)]

use std::collections::HashMap;

use anyhow::Result;

use crate::config::RikuPaths;
use crate::plugin_env::Stored;
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
    let plugin_run_env = match procfile_deploy::runtime_env_for(app, paths, env, &plugin.name) {
        Ok(plugin_run_env) => plugin_run_env,
        Err(e) => return Err(e),
    };
    let env_path = paths.env_root.join(app);
    let ctx = RuntimeContext {
        app,
        app_path: &app_path,
        env_path: &env_path,
        riku_root: &paths.riku_root,
        app_env: &plugin_run_env,
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
        Ok(Stored::Current(stored)) => stored,
        Ok(Stored::Stale) => {
            display::detail("ENV or settings changed since the deploy; running the plugin's env");
            match rerun_and_store(app, paths, &env) {
                Ok(fresh) => fresh,
                Err(e) => return Err(e),
            }
        }
        Ok(Stored::Missing) => {
            display::detail(
                "no runtime plugin env stored by the last deploy; running the plugin's env",
            );
            match rerun_and_store(app, paths, &env) {
                Ok(fresh) => fresh,
                Err(e) => return Err(e),
            }
        }
        Err(e) => return Err(e),
    };
    env.extend(plugin_env);
    Ok(env)
}

/// Runs the plugin's `env` and stores its output with the current config
/// digest, so the next restart with unchanged config skips it.
fn rerun_and_store(
    app: &str,
    paths: &RikuPaths,
    env: &HashMap<String, String>,
) -> Result<HashMap<String, String>> {
    let fresh = match run_plugin_env(app, paths, env) {
        Ok(fresh) => fresh,
        Err(e) => return Err(e),
    };
    match plugin_env::save(app, paths, &fresh) {
        Ok(()) => Ok(fresh),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
#[path = "restart_env_tests.rs"]
mod tests;
