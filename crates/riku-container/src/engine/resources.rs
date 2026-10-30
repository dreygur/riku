//! Per app networks and named volumes.
//!
//! Existence is read from an exact name match in a listing, never from the
//! exit code of an inspect call, so a non zero exit always means the engine
//! failed and is never taken for "absent".

use anyhow::{anyhow, Result};

use super::Engine;
use crate::types::NetworkName;

fn lists_name(listing: &str, name: &str) -> bool {
    listing.lines().any(|line| line.trim() == name)
}

impl Engine {
    fn has_listed(&self, kind: &str, name: &str) -> Result<bool> {
        let args = [
            kind.into(),
            "ls".into(),
            "--filter".into(),
            format!("name={name}").into(),
            "--format".into(),
            "{{.Name}}".into(),
        ];
        match self.output(&args) {
            Ok(listing) => Ok(lists_name(&listing, name)),
            Err(e) => Err(anyhow!("cannot look up {kind} {name:?}: {e}")),
        }
    }

    /// Create the app's network unless it already exists.
    pub fn ensure_network(&self, name: &NetworkName) -> Result<()> {
        match self.has_listed("network", name.as_str()) {
            Ok(true) => return Ok(()),
            Ok(false) => {}
            Err(e) => return Err(e),
        }
        self.output(&["network".into(), "create".into(), name.as_str().into()])
            .map(|_| ())
    }

    /// Whether a volume with exactly this name exists.
    pub fn volume_exists(&self, name: &str) -> Result<bool> {
        self.has_listed("volume", name)
    }
}

#[cfg(test)]
#[path = "resources_tests.rs"]
mod tests;
