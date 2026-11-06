//! Deploying a Procfile app: its runtime plugin builds it, and the riku
//! supervisor runs the Procfile processes it describes.

// Project rule 16 forbids `?`, and this lint asks for exactly that rewrite.
#![allow(clippy::question_mark)]

use std::collections::HashMap;
use std::path::Path;

use anyhow::{anyhow, Result};

use crate::config::RikuPaths;
use crate::plugins::runtime::{RuntimeContext, RuntimePlugin};
use crate::util::{deploy_logger::DeployLogger, echo, parse_procfile};
use crate::{env_setup, hooks, workers};

/// What the runtime plugin's build left for the start step.
struct PluginOutput {
    env: HashMap<String, String>,
    start_cmd: Option<String>,
}

/// The app's Procfile entries. An app with no Procfile runs its runtime
/// plugin's start command as `web`; whether it has one is checked once the
/// plugin is known.
fn read_workers(
    app: &str,
    app_path: &Path,
    dlog: &mut DeployLogger,
) -> Result<HashMap<String, String>> {
    match parse_procfile(&app_path.join("Procfile")) {
        Ok(workers) => Ok(workers.unwrap_or_default()),
        Err(e) => {
            let reason = format!("Invalid Procfile for app '{app}': {e}. Deploy aborted.");
            dlog.log_error(&reason);
            Err(anyhow!(reason))
        }
    }
}

fn detect_runtime(
    app: &str,
    app_path: &Path,
    paths: &RikuPaths,
    env: &HashMap<String, String>,
) -> Result<RuntimePlugin> {
    let plugins = crate::plugins::runtime::discover(&paths.plugin_root);
    match crate::plugins::runtime::detect(&plugins, app_path, env) {
        Ok(Some(plugin)) => Ok(plugin),
        Ok(None) => Err(anyhow!(
            "No runtime plugin matched '{app}'. Run 'riku install-plugins' or set RIKU_RUNTIME=<name> in the app env."
        )),
        Err(e) => Err(e),
    }
}

/// Pre build hooks, then the plugin's build, env and start subcommands.
fn build_with_plugin(
    plugin: &RuntimePlugin,
    ctx: &RuntimeContext,
    paths: &RikuPaths,
    dlog: &mut DeployLogger,
) -> Result<PluginOutput> {
    dlog.log_raw("Running pre-build hooks");
    if let Err(e) = hooks::run_pre_build(
        ctx.app,
        ctx.app_path,
        paths,
        Some(&plugin.name),
        ctx.app_env,
    ) {
        return Err(e);
    }
    dlog.log_raw("Building application");
    if let Err(e) = crate::plugins::runtime::build(plugin, ctx) {
        return Err(e);
    }
    let env = match crate::plugins::runtime::get_env(plugin, ctx) {
        Ok(env) => env,
        Err(e) => return Err(e),
    };
    match crate::plugins::runtime::get_start_cmd(plugin, ctx) {
        Ok(start_cmd) => Ok(PluginOutput { env, start_cmd }),
        Err(e) => Err(e),
    }
}

/// Post build hooks, the `release` command, then worker configs and the
/// processes themselves.
fn release_and_start(
    app: &str,
    paths: &RikuPaths,
    runtime: &str,
    workers: &HashMap<String, String>,
    start_cmd: Option<&str>,
    env: &HashMap<String, String>,
    dlog: &mut DeployLogger,
) -> Result<()> {
    let app_path = paths.app_root.join(app);
    dlog.log_raw("Running post-build hooks");
    if let Err(e) = hooks::run_post_build(app, &app_path, paths, Some(runtime), env) {
        return Err(e);
    }
    if let Some(release_cmd) = workers.get("release") {
        dlog.log("Running release command");
        if let Err(e) = env_setup::run_release(release_cmd, &app_path) {
            return Err(e);
        }
    }
    if let Err(e) = env_setup::write_live_env(app, paths, env) {
        return Err(e);
    }
    crate::supervisor_ctl::start_supervisor(paths);
    if let Err(e) = workers::create_workers_generic(app, &app_path, env, paths, start_cmd) {
        return Err(e);
    }
    dlog.log("Starting application processes");
    crate::spawn_app(app, paths)
}

/// Build and start a Procfile app, after applying `deltas` to its copy
/// counts. Plugin provided env is merged into `env`, so the post deploy hook
/// sees what the processes run with. Returns the runtime plugin's name.
pub(crate) fn deploy_procfile_app(
    app: &str,
    paths: &RikuPaths,
    deltas: &HashMap<String, i64>,
    env: &mut HashMap<String, String>,
    dlog: &mut DeployLogger,
) -> Result<String> {
    let app_path = paths.app_root.join(app);
    let mut workers = match read_workers(app, &app_path, dlog) {
        Ok(workers) => workers,
        Err(e) => return Err(e),
    };
    if let Err(e) = workers::apply_scaling_deltas(app, paths, deltas, &workers) {
        return Err(e);
    }
    if let Some(preflight_cmd) = workers.remove("preflight") {
        dlog.log("Running preflight command");
        env_setup::run_preflight(&preflight_cmd, &app_path);
    }
    dlog.log_raw("Running pre-deploy hooks");
    if let Err(e) = hooks::run_pre_deploy(app, &app_path, paths, env) {
        return Err(e);
    }
    let plugin = match detect_runtime(app, &app_path, paths, env) {
        Ok(plugin) => plugin,
        Err(e) => return Err(e),
    };
    dlog.log(&format!("Detected runtime: {}", plugin.name));
    echo(&format!("-----> {} app detected.", plugin.name), "green");
    let env_path = paths.env_root.join(app);
    let ctx = RuntimeContext {
        app,
        app_path: &app_path,
        env_path: &env_path,
        riku_root: &paths.riku_root,
        app_env: env,
    };
    let output = match build_with_plugin(&plugin, &ctx, paths, dlog) {
        Ok(output) => output,
        Err(e) => return Err(e),
    };
    if workers.is_empty() && output.start_cmd.is_none() {
        let reason = format!(
            "No Procfile for app '{app}' and runtime '{}' gave no start command. Deploy aborted.",
            plugin.name
        );
        dlog.log_error(&reason);
        return Err(anyhow!(reason));
    }
    env.extend(output.env);
    let start_cmd = output.start_cmd.as_deref();
    match release_and_start(app, paths, &plugin.name, &workers, start_cmd, env, dlog) {
        Ok(()) => Ok(plugin.name),
        Err(e) => Err(e),
    }
}
