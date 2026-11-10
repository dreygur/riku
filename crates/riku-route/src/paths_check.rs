//! The gate an `apply_paths` request passes before the helper renders it.

use std::collections::BTreeSet;

use anyhow::{anyhow, Result};
use riku_container::types::AppName;

use crate::check::backend;
use crate::schema::{Listen, PathApp, RouteBackend};

const FIRST_UNPRIVILEGED_PORT: u16 = 1024;
const CADDY_ADMIN_PORT: u16 = 2019;

fn checked_name(raw: &str) -> Result<AppName> {
    match AppName::try_from(raw) {
        Ok(name) if name.as_str() == raw => Ok(name),
        Ok(_) => Err(anyhow!("app {raw:?} is not written as a valid name")),
        Err(e) => Err(anyhow!("app {raw:?}: {e}")),
    }
}

#[allow(clippy::question_mark)]
fn checked_app(entry: &PathApp) -> Result<(AppName, Vec<RouteBackend>)> {
    let name = match checked_name(&entry.app) {
        Ok(name) => name,
        Err(e) => return Err(e),
    };
    if entry.backends.is_empty() {
        return Err(anyhow!("app '{name}' has no backend"));
    }
    if let Err(e) = entry.backends.iter().try_for_each(backend) {
        return Err(anyhow!("app '{name}': {e}"));
    }
    Ok((name, entry.backends.clone()))
}

/// Refuse a path site that is not safe to render: a privileged shared port,
/// an app name `AppName` refuses (which keeps every name off the `_paths`
/// file name), an app listed twice, or a backend the route gate refuses.
#[allow(clippy::question_mark)]
pub fn check_paths(listen: &Listen, apps: &[PathApp]) -> Result<Vec<(AppName, Vec<RouteBackend>)>> {
    if listen.port < FIRST_UNPRIVILEGED_PORT || listen.port == CADDY_ADMIN_PORT {
        return Err(anyhow!(
            "shared port {} is refused; use {FIRST_UNPRIVILEGED_PORT} and above, not {CADDY_ADMIN_PORT}",
            listen.port
        ));
    }
    let mut seen = BTreeSet::new();
    let mut checked = Vec::with_capacity(apps.len());
    for entry in apps {
        if !seen.insert(entry.app.as_str()) {
            return Err(anyhow!("app '{}' is listed twice", entry.app));
        }
        match checked_app(entry) {
            Ok(app) => checked.push(app),
            Err(e) => return Err(e),
        }
    }
    Ok(checked)
}

#[cfg(test)]
#[path = "paths_check_tests.rs"]
mod tests;
