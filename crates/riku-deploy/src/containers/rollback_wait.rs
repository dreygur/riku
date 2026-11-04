//! Waiting for restored old web workers to answer during migration rollback.
//!
//! # Security model
//!
//! Restored worker configs come from riku's own `workers-enabled` directory.
//! Health probes only connect to `127.0.0.1` on the port recorded in the
//! worker's own configuration.

use anyhow::{anyhow, Result};

use crate::config::RikuPaths;
use crate::rolling::health::{wait_healthy, HealthTarget};
use crate::router::web_copies;

/// Wait for every restored old web worker port to answer before traffic is
/// routed to it again.
pub(crate) fn wait_restored_workers(app: &str, paths: &RikuPaths) -> Result<()> {
    let copies = match web_copies(app, paths) {
        Ok(copies) => copies,
        Err(e) => return Err(e),
    };
    for config in &copies {
        let port = match config
            .env
            .get("PORT")
            .map(|port| port.trim().parse::<u16>())
        {
            Some(Ok(port)) => port,
            _ => {
                return Err(anyhow!(
                    "PORT of {} is not a port number",
                    config.worker.name()
                ))
            }
        };
        if let Err(e) = wait_healthy(&HealthTarget::Port(port), None) {
            return Err(e);
        }
    }
    Ok(())
}
