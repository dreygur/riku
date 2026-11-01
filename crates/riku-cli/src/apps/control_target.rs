//! Telling container apps apart for the commands that act on them per
//! service: `riku restart`, `riku stop` and `riku ps --scale`.
//!
//! # Security model
//!
//! The app name was already checked by `exit_if_invalid`; the service name
//! comes from the command line and is validated into a [`ServiceName`] before
//! it reaches the engine.

use anyhow::{anyhow, Result};

use crate::config::RikuPaths;
use crate::deploy::containers::{runs_containers, AppName, ServiceName};

/// What a control command acts on.
pub(crate) enum ControlTarget {
    /// A container app, and the one service named on the command line
    /// (`None` means every service).
    Containers {
        app: AppName,
        service: Option<ServiceName>,
    },
    /// A Procfile app whose processes the supervisor runs.
    Procfile,
}

fn service_name(raw: Option<&str>) -> Result<Option<ServiceName>> {
    match raw {
        Some(raw) => match ServiceName::try_from(raw) {
            Ok(service) => Ok(Some(service)),
            Err(e) => Err(e.context(format!("invalid service argument '{raw}'"))),
        },
        None => Ok(None),
    }
}

/// Classify `app` for a command that may name one of its services. Naming a
/// service of a Procfile app is refused.
#[allow(clippy::question_mark)]
pub(crate) fn control_target(
    paths: &RikuPaths,
    app: &str,
    service: Option<&str>,
) -> Result<ControlTarget> {
    let containers = match runs_containers(app, paths) {
        Ok(containers) => containers,
        Err(e) => return Err(e),
    };
    if !containers {
        return match service {
            None => Ok(ControlTarget::Procfile),
            Some(name) => Err(anyhow!(
                "'{app}' is not a container app, so it has no service '{name}'; leave the service out"
            )),
        };
    }
    let app = match AppName::try_from(app) {
        Ok(app) => app,
        Err(e) => return Err(e),
    };
    match service_name(service) {
        Ok(service) => Ok(ControlTarget::Containers { app, service }),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
#[path = "control_target_tests.rs"]
mod tests;
