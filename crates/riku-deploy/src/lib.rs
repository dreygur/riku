//! Deployment orchestration module.
//!
//! # Security Model
//!
//! **Anyone with git push access to a riku server can execute arbitrary commands
//! on the host.** Procfile commands (web, worker, preflight, release) are run via
//! `sh -c` as the riku user with no sandboxing. This is inherent to the PaaS model.
//!
//! Operators MUST:
//! - Only grant SSH access to trusted users
//! - Run riku under a dedicated unprivileged user account
//! - Consider additional isolation (containers, namespaces) for untrusted workloads
//!
//! Input validation is applied to app names, environment variables, and plugin
//! names to prevent path traversal and injection, but deployed application code
//! itself runs with full user-level privileges.

use anyhow::Result;
use std::collections::HashMap;
use std::fs;

use crate::config::RikuPaths;
use crate::util::{deploy_logger::DeployLogger, echo, parse_procfile};

// Dependency crates aliased as their former module names.
pub(crate) use riku_caddy as caddy;
pub(crate) use riku_config as config;
pub(crate) use riku_error as error;
pub(crate) use riku_nginx as nginx;
pub(crate) use riku_plugins as plugins;
pub(crate) use riku_supervisor as supervisor;
pub(crate) use riku_util as util;

// Deployment infrastructure modules (kept in binary)
pub mod backup;
pub mod container_runtime; // used by `riku container` CLI commands
pub mod env_setup;
pub mod git_ops;
pub mod hooks;
pub mod lock;
pub mod releases;
pub mod router;
pub mod scaling;
pub mod service_update;
pub mod supervisor_ctl;
pub mod teardown;
pub mod worker_control;
pub mod workers;

pub use supervisor_ctl::spawn_app;
pub use workers::create_workers_generic;

/// Deploy an app: sync repo, detect runtime plugin, build, create workers, start processes.
#[allow(clippy::question_mark)]
pub fn do_deploy(
    app: &str,
    paths: &RikuPaths,
    deltas: &HashMap<String, i64>,
    newrev: Option<&str>,
) -> Result<()> {
    let app_path = paths.app_root.join(app);

    if !app_path.exists() {
        return Err(anyhow::anyhow!(
            "App '{}' not found at {}",
            app,
            app_path.display()
        ));
    }

    // Held for the rest of this function: serializes concurrent deploys of
    // the *same* app (e.g. a second git push landing mid-deploy, or a
    // dashboard-triggered redeploy racing a git push) without blocking
    // deploys of other apps. See lock.rs for what breaks without this.
    let _deploy_lock = match lock::acquire(app, paths) {
        Ok(lock) => lock,
        Err(e) => return Err(e),
    };

    let deploy_log_path = paths.deploy_log_file(app);
    let mut dlog = match DeployLogger::new(&deploy_log_path) {
        Ok(dlog) => dlog,
        Err(e) => return Err(e),
    };

    dlog.log(&format!("Deploying app '{}'", app));

    // Sync working tree with the pushed revision.
    dlog.log_raw(&format!("Syncing repo to {}", newrev.unwrap_or("HEAD")));
    if let Err(e) = git_ops::sync_app_repo(&app_path, newrev) {
        return Err(e);
    }

    // Ensure log directory exists.
    let log_path = paths.log_root.join(app);
    if !log_path.exists() {
        if let Err(e) = fs::create_dir_all(&log_path) {
            return Err(e.into());
        }
    }

    // An app with no Procfile runs its runtime plugin's start command as
    // `web`; whether it has one is checked once the plugin is known.
    let mut workers = match parse_procfile(&app_path.join("Procfile")) {
        Ok(workers) => workers.unwrap_or_default(),
        Err(e) => {
            let reason = format!("Invalid Procfile for app '{app}': {e}. Deploy aborted.");
            dlog.log_error(&reason);
            return Err(anyhow::anyhow!(reason));
        }
    };

    // Apply scaling deltas if any.
    if let Err(e) = workers::apply_scaling_deltas(app, paths, deltas, &workers) {
        return Err(e);
    }

    // Run preflight command if present.
    if let Some(preflight_cmd) = workers.remove("preflight") {
        dlog.log("Running preflight command");
        env_setup::run_preflight(&preflight_cmd, &app_path);
    }

    // Load app environment variables.
    let env_file = paths.env_root.join(app).join("ENV");
    let mut env: HashMap<String, String> = HashMap::new();
    if env_file.exists() {
        if let Err(e) = crate::util::parse_settings(&env_file, &mut env) {
            return Err(e);
        }
    }

    // Validate environment variables and print warnings.
    let warnings = crate::util::validate_env_vars(&env);
    crate::util::print_env_warnings(&warnings);
    for w in &warnings {
        dlog.log_warn(w);
    }
    for (old, new) in crate::util::legacy_env::legacy_keys(&env) {
        let hint = format!("ENV key {old} is no longer read, rename it to {new}");
        echo(&hint, "yellow");
        dlog.log_raw(&format!("[WARN] {hint}"));
    }

    // pre-deploy hook (failures abort the deploy).
    dlog.log_raw("Running pre-deploy hooks");
    if let Err(e) = hooks::run_pre_deploy(app, &app_path, paths, &env) {
        return Err(e);
    }

    // Detect runtime plugin.
    let plugins = crate::plugins::runtime::discover(&paths.plugin_root);
    let runtime_plugin = match crate::plugins::runtime::detect(&plugins, &app_path, &env) {
        Ok(Some(plugin)) => plugin,
        Ok(None) => {
            return Err(anyhow::anyhow!(
                "No runtime plugin matched '{}'. Run 'riku install-plugins' or set RIKU_RUNTIME=<name> in the app env.",
                app
            ))
        }
        Err(e) => return Err(e),
    };

    let runtime_name = runtime_plugin.name.clone();
    dlog.log(&format!("Detected runtime: {}", runtime_name));
    echo(&format!("-----> {} app detected.", runtime_name), "green");

    // Run plugin build pipeline, collect env and start_cmd before mutating env.
    let (plugin_env, start_cmd) = {
        let ctx = crate::plugins::runtime::RuntimeContext {
            app,
            app_path: &app_path,
            env_path: &paths.env_root.join(app),
            riku_root: &paths.riku_root,
            app_env: &env,
        };

        // pre-build hook (failures abort the deploy).
        dlog.log_raw("Running pre-build hooks");
        if let Err(e) = hooks::run_pre_build(app, &app_path, paths, Some(&runtime_name), &env) {
            return Err(e);
        }

        // Build via runtime plugin.
        dlog.log_raw("Building application");
        if let Err(e) = crate::plugins::runtime::build(&runtime_plugin, &ctx) {
            return Err(e);
        }

        let plugin_env = match crate::plugins::runtime::get_env(&runtime_plugin, &ctx) {
            Ok(plugin_env) => plugin_env,
            Err(e) => return Err(e),
        };
        let start_cmd = match crate::plugins::runtime::get_start_cmd(&runtime_plugin, &ctx) {
            Ok(start_cmd) => start_cmd,
            Err(e) => return Err(e),
        };

        (plugin_env, start_cmd)
        // ctx (and its &env borrow) is dropped here
    };

    if workers.is_empty() && start_cmd.is_none() {
        let reason = format!(
            "No Procfile for app '{app}' and runtime '{runtime_name}' gave no start command. Deploy aborted."
        );
        dlog.log_error(&reason);
        return Err(anyhow::anyhow!(reason));
    }

    // Merge plugin-provided env vars into the app env.
    env.extend(plugin_env);

    // post-build hook (failures abort the deploy, sees merged env).
    dlog.log_raw("Running post-build hooks");
    if let Err(e) = hooks::run_post_build(app, &app_path, paths, Some(&runtime_name), &env) {
        return Err(e);
    }

    // Run release command if present.
    if let Some(release_cmd) = workers.get("release") {
        dlog.log("Running release command");
        if let Err(e) = env_setup::run_release(release_cmd, &app_path) {
            return Err(e);
        }
    }

    // Write LIVE_ENV with resolved environment.
    if let Err(e) = env_setup::write_live_env(app, paths, &env) {
        return Err(e);
    }

    // Create supervisor worker configs.
    if let Err(e) = create_workers_generic(app, &app_path, &env, paths, start_cmd.as_deref()) {
        return Err(e);
    }

    // Start the application processes.
    dlog.log("Starting application processes");
    if let Err(e) = spawn_app(app, paths) {
        return Err(e);
    }

    // post-deploy hook (non-fatal: failures are warnings only).
    dlog.log_raw("Running post-deploy hooks");
    let _ = hooks::run_post_deploy(app, &app_path, paths, Some(&runtime_name), &env);

    dlog.log(&format!("Deploy of '{}' complete", app));

    // Record the deployed revision for `riku rollback` (best-effort).
    if let Some(sha) = git_ops::head_sha(&app_path) {
        if let Err(e) = releases::ReleaseLog::new(paths).record(app, &sha) {
            tracing::warn!("could not record release for '{}': {}", app, e);
        }
    }

    // Make the "it works" moment unmissable for whoever just ran `git push`.
    echo(&format!("-----> {} deployed!", app), "green");
    match env.get("RIKU_DOMAINS").filter(|d| !d.is_empty()) {
        Some(domain) => {
            let scheme = if crate::util::flag::https_only(&env) {
                "https"
            } else {
                "http"
            };
            echo(&format!("       Live at {}://{}", scheme, domain), "green");
        }
        None => echo(
            &format!(
                "       Add a domain: riku config set {} RIKU_DOMAINS=example.com",
                app
            ),
            "",
        ),
    }
    Ok(())
}

/// Roll an app back to a previous release by redeploying a prior commit. With
/// no `to`, targets the most recent release before the current one.
pub fn rollback(app: &str, paths: &RikuPaths, to: Option<&str>) -> Result<()> {
    let target = match to {
        Some(sha) => sha.to_string(),
        None => releases::ReleaseLog::new(paths)
            .previous(app)
            .ok_or_else(|| {
                anyhow::anyhow!("no previous release recorded for '{}' to roll back to", app)
            })?,
    };
    let short: String = target.chars().take(12).collect();
    echo(&format!("Rolling back '{}' to {}", app, short), "green");
    do_deploy(app, paths, &HashMap::new(), Some(&target))
}
