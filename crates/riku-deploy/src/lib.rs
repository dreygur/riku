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

use crate::config::RikuPaths;
use crate::util::deploy_logger::DeployLogger;

// Dependency crates aliased as their former module names.
pub(crate) use riku_config as config;
pub(crate) use riku_error as error;
pub(crate) use riku_nginx as nginx;
pub(crate) use riku_plugins as plugins;
pub(crate) use riku_supervisor as supervisor;
pub(crate) use riku_util as util;

// Deployment infrastructure modules (kept in binary)
mod app_command;
pub mod app_restart;
pub mod app_stop;
pub mod backup;
mod checkout_restore;
pub mod containers;
mod deploy_env;
mod deploy_finish;
pub mod env_setup;
#[cfg(test)]
mod fake_supervisor;
pub mod git_ops;
pub mod hooks;
mod live_urls;
pub mod lock;
#[cfg(test)]
mod mock_engine;
mod pipeline;
mod procfile_deploy;
mod progress;
pub mod releases;
mod restart_env;
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

/// Checks the app exists, then takes its deploy lock and opens its deploy log.
#[allow(clippy::question_mark)]
fn prepare(app: &str, paths: &RikuPaths) -> Result<(std::fs::File, DeployLogger)> {
    let app_path = paths.app_root.join(app);
    if !app_path.exists() {
        return Err(anyhow::anyhow!(
            "App '{}' not found at {}",
            app,
            app_path.display()
        ));
    }
    let lock = match lock::acquire_for_deploy(app, paths) {
        Ok(lock) => lock,
        Err(e) => return Err(e),
    };
    match DeployLogger::new(&paths.deploy_log_file(app)) {
        Ok(dlog) => Ok((lock, dlog)),
        Err(e) => Err(e),
    }
}

/// Deploy an app: sync the checkout to `newrev`, then run it as riku managed
/// containers when it has a compose file or a Dockerfile, otherwise build it
/// with its runtime plugin and start its Procfile processes.
///
/// `deltas` change Procfile process counts and are refused for container
/// apps. Holds the app's deploy lock throughout. Progress and any failure
/// are printed on the output rail (a failure with how to retry); the error
/// is still returned.
pub fn do_deploy(
    app: &str,
    paths: &RikuPaths,
    deltas: &HashMap<String, i64>,
    newrev: Option<&str>,
) -> Result<()> {
    // Held for the rest of this function: serializes concurrent deploys of
    // the *same* app (e.g. a second git push landing mid-deploy, or a
    // dashboard-triggered redeploy racing a git push) without blocking
    // deploys of other apps. See lock.rs for what breaks without this.
    let (_deploy_lock, dlog) = match prepare(app, paths) {
        Ok(prepared) => prepared,
        Err(e) => {
            let reason = format!("{e:#}");
            util::display::error(&reason);
            util::display::mark_failure_shown(&reason);
            return Err(e);
        }
    };
    let mut progress = progress::DeployProgress::new(app, paths, dlog);
    match pipeline::run(app, paths, deltas, newrev, &mut progress) {
        Ok(()) => Ok(()),
        Err(e) => {
            restore_checkout(app, paths, &mut progress);
            let repo = paths.git_root.join(format!("{app}.git"));
            let branch = git_ops::deploy_branch(&repo).unwrap_or_else(|| "main".to_string());
            progress.failed(&e, &branch);
            Err(e)
        }
    }
}

/// After a failed deploy, put the checkout back at the release that still
/// runs. Recorded in the deploy log; only a failure to do so is shown.
///
/// Once worker configs were rewritten they point at the new checkout, so it
/// stays: resetting it would leave the new configs running the old code.
fn restore_checkout(app: &str, paths: &RikuPaths, progress: &mut progress::DeployProgress) {
    if progress.processes_changed() {
        progress
            .log
            .log_raw("Checkout kept at the pushed commit: worker configs already changed");
        return;
    }
    match checkout_restore::restore_last_release(app, paths) {
        Ok(Some(sha)) => progress
            .log
            .log_raw(&format!("Checkout reset to the last release {sha}")),
        Ok(None) => {}
        Err(e) => {
            let reason = format!("could not reset the checkout to the last release: {e:#}");
            progress.log.log_raw(&format!("[WARN] {reason}"));
            util::display::warn(&reason);
        }
    }
}

/// Roll an app back to a previous release by redeploying a prior commit. With
/// no `to`, targets the most recent release before the current one.
pub fn rollback(app: &str, paths: &RikuPaths, to: Option<&str>) -> Result<()> {
    let target = match to {
        Some(sha) => sha.to_string(),
        None => match releases::ReleaseLog::new(paths).previous(app) {
            Some(sha) => sha,
            None => {
                return Err(anyhow::anyhow!(
                    "no previous release recorded for '{app}' to roll back to"
                ))
            }
        },
    };
    do_deploy(app, paths, &HashMap::new(), Some(&target))
}
