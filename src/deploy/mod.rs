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

// Deployment infrastructure modules (kept in binary)
pub mod backup;
pub mod container_runtime; // used by `riku container` CLI commands
pub mod env_setup;
pub mod git_ops;
pub mod hooks;
pub(crate) mod lock;
pub mod releases;
pub mod scaling;
pub mod supervisor_ctl;
pub mod workers;

pub use supervisor_ctl::spawn_app;
pub use workers::create_workers_generic;

/// Deploy an app: sync repo, detect runtime plugin, build, create workers, start processes.
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
    let _deploy_lock = lock::acquire(app, paths)?;

    let deploy_log_path = paths.deploy_log_file(app);
    let mut dlog = DeployLogger::new(&deploy_log_path)?;

    dlog.log(&format!("Deploying app '{}'", app));
    echo(&format!("-----> Deploying app '{}'", app), "green");

    // Sync working tree with the pushed revision.
    dlog.log_raw(&format!("Syncing repo to {}", newrev.unwrap_or("HEAD")));
    git_ops::sync_app_repo(&app_path, newrev)?;

    // Ensure log directory exists.
    let log_path = paths.log_root.join(app);
    if !log_path.exists() {
        fs::create_dir_all(&log_path)?;
    }

    // Parse Procfile (needed for preflight/release commands and deploy abort guard).
    let mut workers = match parse_procfile(&app_path.join("Procfile")) {
        Some(w) if !w.is_empty() => w,
        _ => {
            dlog.log_error(&format!(
                "Invalid or missing Procfile for app '{}'. Deploy aborted.",
                app
            ));
            return Err(anyhow::anyhow!(
                "Invalid or missing Procfile for app '{}'. Deploy aborted.",
                app
            ));
        }
    };

    // Apply scaling deltas if any.
    workers::apply_scaling_deltas(app, paths, deltas, &workers)?;

    // Run preflight command if present.
    if let Some(preflight_cmd) = workers.remove("preflight") {
        dlog.log("Running preflight command");
        env_setup::run_preflight(&preflight_cmd, &app_path);
    }

    // Load app environment variables.
    let env_file = paths.env_root.join(app).join("ENV");
    let mut env: HashMap<String, String> = HashMap::new();
    if env_file.exists() {
        crate::util::parse_settings(&env_file, &mut env)?;
    }

    // Validate environment variables and print warnings.
    let warnings = crate::util::validate_env_vars(&env);
    crate::util::print_env_warnings(&warnings);
    for w in &warnings {
        dlog.log_warn(w);
    }

    // pre-deploy hook (failures abort the deploy).
    dlog.log_raw("Running pre-deploy hooks");
    hooks::run_pre_deploy(app, &app_path, paths, &env)?;

    // Detect runtime plugin.
    let plugins = crate::plugins::runtime::discover(&paths.plugin_root);
    let runtime_plugin = crate::plugins::runtime::detect(&plugins, &app_path, &env)?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No runtime plugin matched '{}'. Run 'riku install-plugins' or set RUNTIME=<name> in the app env.",
                app
            )
        })?;

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
        hooks::run_pre_build(app, &app_path, paths, Some(&runtime_name), &env)?;

        // Build via runtime plugin.
        dlog.log_raw("Building application");
        crate::plugins::runtime::build(&runtime_plugin, &ctx)?;

        let plugin_env = crate::plugins::runtime::get_env(&runtime_plugin, &ctx)?;
        let start_cmd = crate::plugins::runtime::get_start_cmd(&runtime_plugin, &ctx)?;

        (plugin_env, start_cmd)
        // ctx (and its &env borrow) is dropped here
    };

    // Merge plugin-provided env vars into the app env.
    env.extend(plugin_env);

    // post-build hook (failures abort the deploy, sees merged env).
    dlog.log_raw("Running post-build hooks");
    hooks::run_post_build(app, &app_path, paths, Some(&runtime_name), &env)?;

    // Run release command if present.
    if let Some(release_cmd) = workers.get("release") {
        dlog.log("Running release command");
        env_setup::run_release(release_cmd, &app_path)?;
    }

    // Write LIVE_ENV with resolved environment.
    env_setup::write_live_env(app, paths, &env)?;

    // Create supervisor worker configs.
    create_workers_generic(app, &app_path, &env, paths, start_cmd.as_deref())?;

    // Start the application processes.
    dlog.log("Starting application processes");
    spawn_app(app, paths)?;

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
    match env.get("NGINX_SERVER_NAME").filter(|d| !d.is_empty()) {
        Some(domain) => {
            let https = env
                .get("NGINX_HTTPS_ONLY")
                .map(|v| matches!(v.as_str(), "1" | "true" | "yes" | "on"))
                .unwrap_or(false);
            let scheme = if https { "https" } else { "http" };
            echo(&format!("       Live at {}://{}", scheme, domain), "green");
        }
        None => echo(
            &format!(
                "       Add a domain: riku config set {} NGINX_SERVER_NAME=example.com",
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
