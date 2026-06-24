//! Concrete [`ControlActions`] backing the supervisor's HTTP control plane.
//!
//! Lives in the provider layer (`cli`) so the supervisor crate can depend
//! *downward* only: the supervisor defines the trait, this implements it by
//! delegating to the existing CLI command functions and deploy services, and
//! the binary injects it into the daemon at startup.

use std::path::Path;

use anyhow::Result;

use crate::config::RikuPaths;
use crate::supervisor::health::ControlActions;

/// Routes control-plane requests to the same command functions `riku <cmd>`
/// uses, so HTTP-triggered actions behave identically to the CLI.
pub struct CliControlActions;

impl ControlActions for CliControlActions {
    fn create_app(&self, paths: &RikuPaths, app: &str) -> Result<()> {
        crate::cli::apps::cmd_apps_create(paths, app)
    }
    fn deploy(&self, paths: &RikuPaths, app: &str) -> Result<()> {
        crate::cli::apps::cmd_deploy(paths, app, None)
    }
    fn restart(&self, paths: &RikuPaths, app: &str) -> Result<()> {
        crate::cli::apps::cmd_restart(paths, app)
    }
    fn stop(&self, paths: &RikuPaths, app: &str) -> Result<()> {
        crate::cli::apps::cmd_stop(paths, app)
    }
    fn destroy(&self, paths: &RikuPaths, app: &str) -> Result<()> {
        crate::cli::apps::cmd_destroy(paths, app)
    }
    fn install_plugins(&self, paths: &RikuPaths, only: Option<Vec<String>>) -> Result<()> {
        crate::cli::apps::cmd_install_plugins(paths, only)
    }
    fn container_export(&self, app: &str, context: &Path, output: &Path) -> Result<()> {
        crate::deploy::container_runtime::build_and_export(app, context, output)
    }
    fn list_client_plugins(&self) -> Result<Vec<String>> {
        crate::cli::client_plugins::list_client_plugins()
    }
}
