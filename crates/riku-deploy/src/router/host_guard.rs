//! Refusing router work on a host whose nginx would not load riku's sites
//! exactly once, which is the state an upgrade leaves until `sudo riku init`
//! is re-run. Caddy and router plugins have no such host step.

use anyhow::Result;

use crate::config::router::Router;
use crate::config::RikuPaths;
use crate::nginx::{HostReadiness, RouteChange};

/// Fails for the nginx router when the host is not ready for `change`. A
/// host without the nginx program passes: there is nothing to keep in step,
/// and the nginx step itself then fails naming the missing program. See
/// [`crate::nginx::check_host_ready`].
pub fn ready_for(router: &Router, paths: &RikuPaths, change: RouteChange) -> Result<()> {
    if !matches!(router, Router::Nginx) {
        return Ok(());
    }
    match crate::nginx::check_host_ready(paths, change) {
        Ok(HostReadiness::Ready) => Ok(()),
        Ok(HostReadiness::NoNginx(reason)) => {
            tracing::debug!("nginx host check skipped: {reason}");
            Ok(())
        }
        Err(e) => Err(e),
    }
}

/// [`ready_for`] the active router. Deploy and destroy call this before they
/// change anything, so a refusal leaves the app as it was.
pub fn ensure_host_ready(paths: &RikuPaths, change: RouteChange) -> Result<()> {
    match Router::load(paths) {
        Ok(router) => ready_for(&router, paths, change),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
#[path = "host_guard_tests.rs"]
mod tests;
