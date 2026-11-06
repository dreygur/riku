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
use crate::util::{deploy_logger::DeployLogger, echo};

// Dependency crates aliased as their former module names.
pub(crate) use riku_config as config;
pub(crate) use riku_error as error;
pub(crate) use riku_nginx as nginx;
pub(crate) use riku_plugins as plugins;
pub(crate) use riku_supervisor as supervisor;
pub(crate) use riku_util as util;

// Deployment infrastructure modules (kept in binary)
pub mod app_stop;
pub mod backup;
pub mod containers;
mod deploy_env;
mod deploy_finish;
pub mod env_setup;
pub mod git_ops;
pub mod hooks;
pub mod lock;
#[cfg(test)]
mod mock_engine;
mod procfile_deploy;
pub mod releases;
pub mod rolling;
pub mod router;
pub mod scaling;
pub mod service_update;
pub mod supervisor_ctl;
pub mod teardown;
pub mod worker_control;
pub mod workers;

pub use supervisor_ctl::spawn_app;
pub use workers::create_workers_generic;

/// Pre deploy hooks, then the container deploy and the app's `LIVE_ENV`.
/// Copy counts of a container app change through `scale_service`, never
/// through deploy deltas.
#[allow(clippy::question_mark)]
fn deploy_container_app(
    app: &str,
    paths: &RikuPaths,
    deltas: &HashMap<String, i64>,
    env: &HashMap<String, String>,
    dlog: &mut DeployLogger,
) -> Result<()> {
    if !deltas.is_empty() {
        return Err(anyhow::anyhow!(
            "'{app}' runs as containers; scale it with riku ps {app} --scale <service>=<copies>"
        ));
    }
    let name = match containers::AppName::try_from(app) {
        Ok(name) => name,
        Err(e) => return Err(e),
    };
    dlog.log_raw("Running pre-deploy hooks");
    if let Err(e) = hooks::run_pre_deploy(app, &paths.app_root.join(app), paths, env) {
        return Err(e);
    }
    dlog.log("Detected runtime: container");
    echo("-----> Container app detected.", "green");
    if let Err(e) = containers::deploy_containers(&name, paths, env) {
        return Err(e);
    }
    env_setup::write_live_env(app, paths, env)
}

/// Deploy an app: sync the checkout to `newrev`, then run it as riku managed
/// containers when it has a compose file or a Dockerfile, otherwise build it
/// with its runtime plugin and start its Procfile processes.
///
/// `deltas` change Procfile process counts and are refused for container
/// apps. Holds the app's deploy lock throughout.
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
    let _deploy_lock = match lock::acquire_for_deploy(app, paths) {
        Ok(lock) => lock,
        Err(e) => return Err(e),
    };

    let mut dlog = match DeployLogger::new(&paths.deploy_log_file(app)) {
        Ok(dlog) => dlog,
        Err(e) => return Err(e),
    };
    dlog.log(&format!("Deploying app '{}'", app));

    dlog.log_raw(&format!("Syncing repo to {}", newrev.unwrap_or("HEAD")));
    if let Err(e) = git_ops::sync_app_repo(&app_path, newrev) {
        return Err(e);
    }

    if let Err(e) = fs::create_dir_all(paths.log_root.join(app)) {
        return Err(anyhow::anyhow!(
            "cannot create the log directory of '{app}': {e}"
        ));
    }

    let mut env = match deploy_env::load_deploy_env(app, paths, &mut dlog) {
        Ok(env) => env,
        Err(e) => return Err(e),
    };

    let is_container = match containers::is_container_app(&app_path, &env) {
        Ok(is_container) => is_container,
        Err(e) => return Err(e),
    };
    let runtime = if is_container {
        // Keys a former Procfile web kind left would pick the port map or
        // uWSGI template for the containers.
        workers::web_env::drop_route_keys(&mut env);
        if let Err(e) = workers::web_env::clear_route_env(app, paths) {
            return Err(e);
        }
        match deploy_container_app(app, paths, deltas, &env, &mut dlog) {
            Ok(()) => containers::CONTAINER_RUNTIME.to_string(),
            Err(e) => return Err(e),
        }
    } else {
        match procfile_deploy::deploy_procfile_app(app, paths, deltas, &mut env, &mut dlog) {
            Ok(runtime) => runtime,
            Err(e) => return Err(e),
        }
    };

    deploy_finish::finish_deploy(app, paths, &runtime, &env, &mut dlog);
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
