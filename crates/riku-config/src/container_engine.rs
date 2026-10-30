//! Container engine choice, stored in `<riku root>/CONTAINER_ENGINE`.
//!
//! The setting is a file for the same reason the router is: git push deploys
//! run without the operator's environment. The file holds either an engine
//! name such as `podman` or an absolute path. It is only read here; the
//! value is validated where it is turned into a program to run.

use std::fs;
use std::io::ErrorKind;

use anyhow::{anyhow, Result};

use crate::RikuPaths;

/// File name of the engine choice, directly under the riku root.
pub const ENGINE_FILE: &str = "CONTAINER_ENGINE";

/// Read the engine setting. A missing or blank file returns `None`, meaning
/// the engine is detected; any other read failure is an error.
pub fn load(paths: &RikuPaths) -> Result<Option<String>> {
    match fs::read_to_string(&paths.container_engine_file) {
        Ok(raw) => {
            let value = raw.trim();
            if value.is_empty() {
                Ok(None)
            } else {
                Ok(Some(value.to_string()))
            }
        }
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(anyhow!(
            "failed to read {ENGINE_FILE} file {}: {e}",
            paths.container_engine_file.display()
        )),
    }
}

#[cfg(test)]
#[path = "container_engine_tests.rs"]
mod tests;
