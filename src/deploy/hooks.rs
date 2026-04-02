//! Plugin lifecycle hook invocation for the deploy pipeline.
//!
//! Each hook stage (pre-deploy, pre-build, post-build, post-deploy) is
//! wrapped in its own function so `mod.rs` stays thin and readable.

use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;

use crate::config::RikuPaths;
use crate::plugins::{HookContext, PluginHook, PluginManager};

fn run_hook(
    hook: &PluginHook,
    app: &str,
    app_path: &Path,
    paths: &RikuPaths,
    runtime_name: Option<&str>,
    app_env: &HashMap<String, String>,
) -> Result<()> {
    let plugin_manager = PluginManager::new(paths);
    let env_path = paths.env_root.join(app);
    let ctx = HookContext {
        app,
        hook,
        app_path,
        env_path: &env_path,
        riku_root: &paths.riku_root,
        runtime: runtime_name,
        app_env,
    };
    plugin_manager.run_hook(&ctx).map(|_| ())
}

/// Run the `pre-deploy` hook.  Failures abort the deploy.
pub fn run_pre_deploy(
    app: &str,
    app_path: &Path,
    paths: &RikuPaths,
    app_env: &HashMap<String, String>,
) -> Result<()> {
    run_hook(&PluginHook::PreDeploy, app, app_path, paths, None, app_env)
}

/// Run the `pre-build` hook.  Failures abort the deploy.
pub fn run_pre_build(
    app: &str,
    app_path: &Path,
    paths: &RikuPaths,
    runtime_name: Option<&str>,
    app_env: &HashMap<String, String>,
) -> Result<()> {
    run_hook(
        &PluginHook::PreBuild,
        app,
        app_path,
        paths,
        runtime_name,
        app_env,
    )
}

/// Run the `post-build` hook.  Failures abort the deploy.
pub fn run_post_build(
    app: &str,
    app_path: &Path,
    paths: &RikuPaths,
    runtime_name: Option<&str>,
    app_env: &HashMap<String, String>,
) -> Result<()> {
    run_hook(
        &PluginHook::PostBuild,
        app,
        app_path,
        paths,
        runtime_name,
        app_env,
    )
}

/// Run the `post-deploy` hook.  Failures are warnings (not fatal).
pub fn run_post_deploy(
    app: &str,
    app_path: &Path,
    paths: &RikuPaths,
    runtime_name: Option<&str>,
    app_env: &HashMap<String, String>,
) -> Result<()> {
    run_hook(
        &PluginHook::PostDeploy,
        app,
        app_path,
        paths,
        runtime_name,
        app_env,
    )
}
