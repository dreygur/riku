//! The control-plane action seam.
//!
//! The supervisor's HTTP control plane (`control.rs`, `plugins.rs`) must drive
//! high-level app lifecycle (deploy/restart/destroy/…) and client-plugin
//! discovery: logic that lives in higher layers (`cli`, `deploy`). To keep the
//! supervisor a *lower* crate that those layers depend on (not the reverse),
//! the handlers call through this trait instead of those crates directly. The
//! binary injects a concrete implementation at startup via
//! [`super::super::daemon::Supervisor::with_actions`].

use std::path::Path;
use std::sync::Arc;

use anyhow::Result;

use riku_config::RikuPaths;

/// What a request to restart one dead container copy did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestartOutcome {
    /// The dead container was removed and its slot started again.
    Restarted,
    /// A deploy of the app holds its lock, so nothing was touched. The caller
    /// tries again later.
    Busy,
    /// The slot no longer holds an exited container: `riku stop`, a scale
    /// down or a deploy removed or replaced it since it died. Nothing was
    /// started, so a stop by the user stays a stop.
    Gone,
}

/// Operations the control plane needs but that live above the supervisor.
pub trait ControlActions: Send + Sync {
    fn create_app(&self, paths: &RikuPaths, app: &str) -> Result<()>;
    fn deploy(&self, paths: &RikuPaths, app: &str) -> Result<()>;
    fn restart(&self, paths: &RikuPaths, app: &str) -> Result<()>;
    fn stop(&self, paths: &RikuPaths, app: &str) -> Result<()>;
    fn destroy(&self, paths: &RikuPaths, app: &str) -> Result<()>;
    fn install_plugins(&self, paths: &RikuPaths, only: Option<Vec<String>>) -> Result<()>;
    fn container_export(&self, app: &str, context: &Path, output: &Path) -> Result<()>;
    fn list_client_plugins(&self) -> Result<Vec<String>>;
    /// Pull a fresh image and recreate a single compose service, without a
    /// full redeploy. Used by the daemon's periodic image-watch check
    /// (see `daemon::image_watch`).
    fn pull_service(&self, paths: &RikuPaths, app: &str, service: &str) -> Result<()>;
    /// Restart one dead container copy for the supervisor's watch.
    ///
    /// Returns [`RestartOutcome::Busy`] when a deploy holds the app lock.
    /// Blocks through the health wait, so callers run it off the main loop.
    fn restart_copy(
        &self,
        paths: &RikuPaths,
        app: &str,
        service: &str,
        copy: u32,
    ) -> Result<RestartOutcome>;
}

/// Default used when no implementation is injected (tests, or a supervisor
/// started without a control plane). Every action reports that the control
/// plane is unavailable rather than doing anything.
pub struct NoopControlActions;

impl NoopControlActions {
    fn unavailable() -> anyhow::Error {
        anyhow::anyhow!("control plane not available in this supervisor")
    }
}

impl ControlActions for NoopControlActions {
    fn create_app(&self, _: &RikuPaths, _: &str) -> Result<()> {
        Err(Self::unavailable())
    }
    fn deploy(&self, _: &RikuPaths, _: &str) -> Result<()> {
        Err(Self::unavailable())
    }
    fn restart(&self, _: &RikuPaths, _: &str) -> Result<()> {
        Err(Self::unavailable())
    }
    fn stop(&self, _: &RikuPaths, _: &str) -> Result<()> {
        Err(Self::unavailable())
    }
    fn destroy(&self, _: &RikuPaths, _: &str) -> Result<()> {
        Err(Self::unavailable())
    }
    fn install_plugins(&self, _: &RikuPaths, _: Option<Vec<String>>) -> Result<()> {
        Err(Self::unavailable())
    }
    fn container_export(&self, _: &str, _: &Path, _: &Path) -> Result<()> {
        Err(Self::unavailable())
    }
    fn list_client_plugins(&self) -> Result<Vec<String>> {
        Err(Self::unavailable())
    }
    fn pull_service(&self, _: &RikuPaths, _: &str, _: &str) -> Result<()> {
        Err(Self::unavailable())
    }
    fn restart_copy(&self, _: &RikuPaths, _: &str, _: &str, _: u32) -> Result<RestartOutcome> {
        Err(Self::unavailable())
    }
}

/// Shared handle threaded through the daemon and into the HTTP handlers.
pub type SharedActions = Arc<dyn ControlActions>;
