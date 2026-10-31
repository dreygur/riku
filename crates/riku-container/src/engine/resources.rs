//! Per app networks and named volumes.
//!
//! Existence is read from an exact name match in a listing, never from the
//! exit code of an inspect call, so a non zero exit always means the engine
//! failed and is never taken for "absent".

use std::collections::BTreeMap;

use anyhow::{anyhow, Result};

use super::Engine;
use crate::types::{check_engine_name, NetworkName};

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

    /// Labels of an existing volume; empty when it has none. A volume that
    /// does not exist is an error.
    pub fn volume_labels(&self, name: &str) -> Result<BTreeMap<String, String>> {
        match check_engine_name("volume name", name) {
            Ok(()) => {}
            Err(e) => return Err(e),
        }
        let args = [
            "volume".into(),
            "inspect".into(),
            "--format".into(),
            "{{json .Labels}}".into(),
            name.into(),
        ];
        let json = match self.output(&args) {
            Ok(json) => json,
            Err(e) => return Err(anyhow!("cannot read labels of volume {name:?}: {e}")),
        };
        match serde_json::from_str::<Option<BTreeMap<String, String>>>(json.trim()) {
            Ok(labels) => Ok(labels.unwrap_or_default()),
            Err(e) => Err(anyhow!(
                "labels of volume {name:?} are not the expected JSON: {e}"
            )),
        }
    }

    /// Remove a network. One that does not exist is not an error.
    pub fn remove_network(&self, name: &str) -> Result<()> {
        match check_engine_name("network name", name) {
            Ok(()) => {}
            Err(e) => return Err(e),
        }
        match self.has_listed("network", name) {
            Ok(true) => {}
            Ok(false) => return Ok(()),
            Err(e) => return Err(e),
        }
        self.output(&["network".into(), "rm".into(), name.into()])
            .map(|_| ())
    }
}

#[cfg(test)]
#[path = "resources_tests.rs"]
mod tests;
