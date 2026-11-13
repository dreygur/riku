//! Concrete [`ControlActions`] backing the supervisor's HTTP control plane.
//!
//! Lives in the provider layer (`cli`) so the supervisor crate can depend
//! *downward* only: the supervisor defines the trait, this implements it by
//! delegating to the existing CLI command functions and deploy services, and
//! the binary injects it into the daemon at startup.

use anyhow::Result;

use crate::config::RikuPaths;
use crate::deploy::containers::{AppName, CopyIndex, ServiceName};
use crate::supervisor::health::{ControlActions, RestartOutcome};
use crate::util::existing_app;

/// Routes control-plane requests to the same command functions `riku <cmd>`
/// uses, so HTTP-triggered actions behave identically to the CLI. The app is
/// checked with [`existing_app`] first, so an unknown app is a plain error
/// and the supervisor's log gets none of the CLI's terminal hints.
pub struct CliControlActions;

impl ControlActions for CliControlActions {
    fn create_app(&self, paths: &RikuPaths, app: &str) -> Result<()> {
        crate::apps::cmd_apps_create(paths, app)
    }
    fn deploy(&self, paths: &RikuPaths, app: &str) -> Result<()> {
        match existing_app(app, &paths.app_root) {
            Ok(app) => crate::apps::cmd_deploy(paths, &app, None),
            Err(e) => Err(e),
        }
    }
    fn restart(&self, paths: &RikuPaths, app: &str) -> Result<()> {
        match existing_app(app, &paths.app_root) {
            Ok(app) => crate::apps::cmd_restart(paths, &app, None),
            Err(e) => Err(e),
        }
    }
    fn stop(&self, paths: &RikuPaths, app: &str) -> Result<()> {
        match existing_app(app, &paths.app_root) {
            Ok(app) => crate::apps::cmd_stop(paths, &app, None),
            Err(e) => Err(e),
        }
    }
    fn destroy(&self, paths: &RikuPaths, app: &str) -> Result<()> {
        match existing_app(app, &paths.app_root) {
            Ok(app) => crate::apps::cmd_destroy(paths, &app, crate::apps::DestroyMode::Safe),
            Err(e) => Err(e),
        }
    }
    fn install_plugins(&self, paths: &RikuPaths, only: Option<Vec<String>>) -> Result<()> {
        crate::apps::cmd_install_plugins(paths, only)
    }
    fn pull_service(&self, paths: &RikuPaths, app: &str, service: &str) -> Result<()> {
        crate::deploy::service_update::pull_service(app, paths, service)
    }
    fn restart_copy(
        &self,
        paths: &RikuPaths,
        app: &str,
        service: &str,
        copy: u32,
    ) -> Result<RestartOutcome> {
        let app_name = match AppName::try_from(app) {
            Ok(name) => name,
            Err(e) => return Err(e.context("cannot restart a copy")),
        };
        let service_name = match ServiceName::try_from(service) {
            Ok(name) => name,
            Err(e) => return Err(e.context(format!("cannot restart a copy of {app_name}"))),
        };
        let copy_index = match CopyIndex::new(copy) {
            Some(idx) => idx,
            None => return Err(anyhow::anyhow!("invalid copy number: {copy}")),
        };
        crate::deploy::containers::restart_copy(&app_name, paths, &service_name, copy_index)
    }
}

#[cfg(test)]
#[path = "control_actions_tests.rs"]
mod tests;
