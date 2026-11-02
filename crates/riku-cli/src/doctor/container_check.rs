//! `riku doctor` checks for the container engine and legacy container apps.
//!
//! Read-only like every other doctor check: opening the engine only runs its
//! probe and `version` commands.

use crate::config::RikuPaths;
use crate::deploy::containers;

use super::Check;

const ENGINE_NAME: &str = "container engine";
const LEGACY_NAME: &str = "legacy container apps";

/// Report the detected container engine and version, or warn if no engine
/// can be opened.
pub fn container_engine(paths: &RikuPaths) -> Check {
    match containers::open_engine(paths) {
        Ok(engine) => {
            let identity = engine.identity();
            Check::ok(
                ENGINE_NAME,
                format!("{} {}", identity.name, identity.version),
            )
        }
        Err(e) => Check::warn(ENGINE_NAME, format!("{e:#}")),
    }
}

/// Warn when any enabled worker still runs an old container plugin command,
/// ok otherwise.
pub fn legacy_containers(paths: &RikuPaths) -> Check {
    let apps = match containers::legacy_apps(paths) {
        Ok(apps) => apps,
        Err(e) => return Check::warn(LEGACY_NAME, format!("{e:#}")),
    };
    if apps.is_empty() {
        return Check::ok(LEGACY_NAME, "none");
    }
    Check::warn(
        LEGACY_NAME,
        format!(
            "{} still run by the old container plugin; redeploy to migrate (short outage, volumes kept)",
            apps.join(", ")
        ),
    )
}

#[cfg(test)]
#[path = "container_check_tests.rs"]
mod tests;
