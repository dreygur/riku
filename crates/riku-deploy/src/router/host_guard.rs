//! Refusing a router install while nginx would load a `*.conf` that is no
//! app's config. Caddy and router plugins have no such host step.

use anyhow::Result;

use crate::config::router::Router;
use crate::config::RikuPaths;
use crate::nginx::HostReadiness;

/// Fails for the nginx router when the host is not ready for an install. A
/// host without the nginx program passes: the nginx step itself then fails
/// naming the missing program. See [`crate::nginx::check_host_ready`].
pub fn ready_for(router: &Router, paths: &RikuPaths) -> Result<()> {
    if !matches!(router, Router::Nginx) {
        return Ok(());
    }
    match crate::nginx::check_host_ready(paths) {
        Ok(HostReadiness::Ready) => Ok(()),
        Ok(HostReadiness::NoNginx(reason)) => {
            tracing::debug!("nginx host check skipped: {reason}");
            Ok(())
        }
        Err(e) => Err(e),
    }
}

/// [`ready_for`] the active router. Deploy calls this before it changes
/// anything, so a refusal leaves the app as it was.
pub fn ensure_host_ready(paths: &RikuPaths) -> Result<()> {
    match Router::load(paths) {
        Ok(router) => ready_for(&router, paths),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
#[path = "host_guard_tests.rs"]
mod tests;
