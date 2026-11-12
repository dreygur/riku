//! Deploying a Procfile app: its runtime plugin builds it, and the riku
//! supervisor runs the Procfile processes it describes.

// Project rule 16 forbids `?`, and this lint asks for exactly that rewrite.
#![allow(clippy::question_mark)]

use std::collections::HashMap;
use std::path::Path;

use anyhow::{anyhow, Result};

use crate::app_command::{run_one_shot, OneShot};
use crate::config::router::Router;
use crate::config::RikuPaths;
use crate::plugins::runtime::{MissingRuntime, RuntimeContext, RuntimePlugin};
use crate::progress::{DeployProgress, Phase};
use crate::util::runtime_markers::{is_bundled_runtime, needed_runtime};
use crate::workers::{is_scalable_kind, read_scaling_count};
use crate::{env_setup, hooks, plugin_env, workers};

/// What the runtime plugin's build left for the start step.
pub(crate) struct PluginOutput {
    /// The plugin's `env` output, kept for `riku restart`.
    pub env: HashMap<String, String>,
    /// The plugin's `start` command, run as `web` when no process is declared.
    pub start_cmd: Option<String>,
}

/// What a finished app deploy reports: the runtime that built it and its
/// process counts (`web 1, worker 2`), empty when riku does not count them.
pub(crate) struct Deployed {
    pub runtime: String,
    pub workers: String,
}

/// `web 1, worker 2` for the long running kinds in `declared`, sorted.
fn process_counts(app: &str, paths: &RikuPaths, declared: &HashMap<String, String>) -> String {
    let mut kinds: Vec<&String> = declared
        .keys()
        .filter(|kind| is_scalable_kind(kind))
        .collect();
    kinds.sort();
    kinds
        .iter()
        .map(|kind| {
            let count = read_scaling_count(paths, app, kind).unwrap_or(1);
            format!("{kind} {count}")
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn install_hint(app: &str, runtime: &str) -> anyhow::Error {
    anyhow!(
        "app '{app}' needs the {runtime} plugin. Install it with: rk plugins install {runtime} \
         (on the server: riku plugins install {runtime}), then push again."
    )
}

/// The installed plugin that builds `app`. When none does and the app needs
/// a runtime riku ships, the error names the command that installs it.
pub(crate) fn detect_runtime(
    app: &str,
    app_path: &Path,
    paths: &RikuPaths,
    env: &HashMap<String, String>,
) -> Result<RuntimePlugin> {
    let plugins = crate::plugins::runtime::discover(&paths.plugin_root);
    match crate::plugins::runtime::detect(&plugins, app_path, env) {
        Ok(Some(plugin)) => Ok(plugin),
        Ok(None) => match needed_runtime(app_path) {
            Some(name) if !plugins.iter().any(|p| p.name == name) => {
                Err(install_hint(app, name))
            }
            _ if crate::router::app_routing::serves_static_site(env) => Err(anyhow!(
                "No runtime plugin matched '{app}'. `routing.static` is set, but riku serves static files only next to a runtime: the app still needs a runtime plugin and a process. See the Static Sites section of the runtimes docs."
            )),
            _ => Err(anyhow!(
                "No runtime plugin matched '{app}'. Run 'riku install-plugins' or set runtime.name in riku.toml (or with riku settings set)."
            )),
        },
        Err(e) => match e.downcast_ref::<MissingRuntime>() {
            Some(missing) if is_bundled_runtime(&missing.name) => {
                Err(install_hint(app, &missing.name))
            }
            _ => Err(e),
        },
    }
}

/// The env the runtime plugin `runtime` runs with: `env` with its version
/// key taken only from riku.toml `[runtime] version`.
pub(crate) fn runtime_env_for(
    app: &str,
    paths: &RikuPaths,
    env: &HashMap<String, String>,
    runtime: &str,
) -> Result<HashMap<String, String>> {
    match crate::config::app_config::runtime_versions(paths, app) {
        Ok(versions) => Ok(crate::config::app_config::runtime_plugin_env(
            env, &versions, runtime,
        )),
        Err(e) => Err(e),
    }
}

/// Pre build hooks, then the plugin's build, env and start subcommands.
fn build_with_plugin(
    plugin: &RuntimePlugin,
    ctx: &RuntimeContext,
    paths: &RikuPaths,
    progress: &mut DeployProgress,
) -> Result<PluginOutput> {
    progress.log.log_raw("Running pre-build hooks");
    if let Err(e) = hooks::run_pre_build(
        ctx.app,
        ctx.app_path,
        paths,
        Some(&plugin.name),
        ctx.app_env,
    ) {
        return Err(e);
    }
    progress.log.log_raw("Building application");
    if let Err(e) = crate::plugins::runtime::build(plugin, ctx, progress.output()) {
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
/// processes themselves. `LIVE_ENV` and the stored plugin env are written
/// just before the processes change, so a restart later sees what they run.
pub(crate) fn release_and_start(
    app: &str,
    paths: &RikuPaths,
    runtime: &str,
    workers: &HashMap<String, String>,
    plugin: &PluginOutput,
    env: &HashMap<String, String>,
    progress: &mut DeployProgress,
) -> Result<String> {
    let app_path = paths.app_root.join(app);
    let start_cmd = plugin.start_cmd.as_deref();
    progress.log.log_raw("Running post-build hooks");
    if let Err(e) = hooks::run_post_build(app, &app_path, paths, Some(runtime), env) {
        return Err(e);
    }
    if let Some(release_cmd) = workers.get("release") {
        progress.begin(Phase::Release);
        if let Err(e) = run_one_shot(
            OneShot::Release,
            release_cmd,
            &app_path,
            env,
            progress.output(),
        ) {
            return Err(e);
        }
        progress.done_timed(release_cmd);
    }
    progress.begin(Phase::Processes);
    if let Err(e) = workers::check_workers(app, Some(workers), env, paths, start_cmd) {
        return Err(e);
    }
    if let Err(e) = env_setup::write_live_env(app, paths, env) {
        return Err(e);
    }
    if let Err(e) = plugin_env::save(app, paths, &plugin.env) {
        return Err(e);
    }
    progress.release_replacing();
    if let Err(e) =
        workers::create_workers_generic(app, &app_path, Some(workers), env, paths, start_cmd)
    {
        return Err(e);
    }
    // An app that declares no processes runs the plugin's start command as web.
    let counts = if workers.is_empty() {
        process_counts(
            app,
            paths,
            &HashMap::from([("web".to_string(), String::new())]),
        )
    } else {
        process_counts(app, paths, workers)
    };
    progress.done(&counts);
    progress.begin(Phase::Route);
    if let Err(e) = crate::spawn_app(app, paths) {
        return Err(e);
    }
    if let Some(router) = route_step(app, env, paths) {
        progress.done(&router);
    }
    Ok(counts)
}

/// The detail of the `Route` step, the active router's name, or `None` for
/// an app the router left out (see `router::has_route`). Like a Procfile
/// without `release`, an app with no route prints no step at all.
///
/// When `has_route` fails here the step prints as it did before the check:
/// the router was already configured, so the step must not fail the deploy.
fn route_step(app: &str, env: &HashMap<String, String>, paths: &RikuPaths) -> Option<String> {
    match crate::router::has_route(app, env, paths) {
        Ok(false) => return None,
        Ok(true) => {}
        Err(e) => tracing::warn!("cannot tell whether '{app}' has a route: {e:#}"),
    }
    match Router::load(paths) {
        Ok(router) => Some(router.name().to_string()),
        Err(_) => Some(String::new()),
    }
}

/// Build and start a Procfile app from its process list (riku.toml or
/// Procfile, empty when it declares none), after applying `deltas` to its
/// copy counts. An app with no processes runs its runtime plugin's start
/// command as `web`. Plugin provided env is merged into `env`, so the post
/// deploy hook sees what the processes run with.
pub(crate) fn deploy_procfile_app(
    app: &str,
    paths: &RikuPaths,
    deltas: &HashMap<String, i64>,
    mut workers: HashMap<String, String>,
    env: &mut HashMap<String, String>,
    progress: &mut DeployProgress,
) -> Result<Deployed> {
    let app_path = paths.app_root.join(app);
    if let Err(e) = workers::apply_scaling_deltas(app, paths, deltas, &workers) {
        return Err(e);
    }
    // Worker creation gets the full list: a list holding only one shot
    // kinds still counts as declared, so no start command is added as web.
    let declared = workers.clone();
    if let Some(preflight_cmd) = workers.remove("preflight") {
        progress.begin(Phase::Preflight);
        if let Err(e) = run_one_shot(
            OneShot::Preflight,
            &preflight_cmd,
            &app_path,
            env,
            progress.output(),
        ) {
            return Err(e);
        }
        progress.done_timed(&preflight_cmd);
    }
    progress.log.log_raw("Running pre-deploy hooks");
    if let Err(e) = hooks::run_pre_deploy(app, &app_path, paths, env) {
        return Err(e);
    }
    progress.begin(Phase::Runtime);
    let plugin = match detect_runtime(app, &app_path, paths, env) {
        Ok(plugin) => plugin,
        Err(e) => return Err(e),
    };
    progress.done(&plugin.name);
    let plugin_run_env = match runtime_env_for(app, paths, env, &plugin.name) {
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
    progress.begin(Phase::Build);
    let output = match build_with_plugin(&plugin, &ctx, paths, progress) {
        Ok(output) => output,
        Err(e) => return Err(e),
    };
    if workers.is_empty() && output.start_cmd.is_none() {
        return Err(anyhow!(
            "no processes declared for app '{app}' (riku.toml or Procfile) and runtime '{}' gave no start command",
            plugin.name
        ));
    }
    progress.done_timed("");
    env.extend(output.env.clone());
    match release_and_start(app, paths, &plugin.name, &declared, &output, env, progress) {
        Ok(workers) => Ok(Deployed {
            runtime: plugin.name,
            workers,
        }),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
#[path = "procfile_deploy_tests.rs"]
mod tests;
