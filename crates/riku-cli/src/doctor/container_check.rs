//! `riku doctor` check for the container engine.
//!
//! Read-only like every other doctor check: opening the engine only runs its
//! probe and `version` commands.

use crate::config::RikuPaths;
use crate::deploy::containers;

use super::Check;

const ENGINE_NAME: &str = "container engine";

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
