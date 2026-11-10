//! Which installer `riku plugins install <source>` uses.
//!
//! A bare name of a plugin riku ships installs that plugin. Anything that
//! looks like a path or a git source goes to the bundle installer. Any other
//! bare name is refused, so a stray directory in the working directory can
//! never be installed by accident under a runtime's name.

use anyhow::{bail, Result};
use riku_util::runtime_markers::BUNDLED_RUNTIMES;

const NOTIFY_BUNDLE: &str = "riku-notify";

/// Where a `plugins install` source goes.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum InstallTarget {
    /// A plugin riku ships, by name.
    Runtime(&'static str),
    /// A bundle from a path or a git source.
    Bundle(String),
}

fn looks_like_bundle_source(source: &str) -> bool {
    source.contains('/') || source.contains(':') || source.ends_with(".git")
}

fn shipped(name: &str) -> Option<&'static str> {
    if name == NOTIFY_BUNDLE {
        return Some(NOTIFY_BUNDLE);
    }
    BUNDLED_RUNTIMES
        .iter()
        .copied()
        .find(|known| *known == name)
}

/// Classify `source`: paths (`./x`, `../x`, `/x`, `dir/x`) and git sources
/// (`github:o/r`, `git@...`, URLs, `*.git`) are bundles; a bare name riku
/// ships is that plugin; any other bare name is an error.
pub(crate) fn install_target(source: &str) -> Result<InstallTarget> {
    if looks_like_bundle_source(source) {
        return Ok(InstallTarget::Bundle(source.to_string()));
    }
    match shipped(source) {
        Some(name) => Ok(InstallTarget::Runtime(name)),
        None => bail!(
            "'{source}' is not a plugin riku ships ({}, {NOTIFY_BUNDLE}); for a local bundle directory write ./{source}",
            BUNDLED_RUNTIMES.join(", ")
        ),
    }
}

#[cfg(test)]
#[path = "install_route_tests.rs"]
mod tests;
