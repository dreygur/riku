//! Container engine choice: `[engine] program` in `settings.toml`, else the
//! legacy `<riku root>/CONTAINER_ENGINE` file that older versions wrote.
//!
//! The setting is a file for the same reason the router is: git push deploys
//! run without the operator's environment. It holds either an engine name
//! such as `podman` or an absolute path. It is only read here; the value is
//! validated where it is turned into a program to run.

use std::fs;
use std::io::ErrorKind;

use anyhow::{anyhow, Result};

use crate::settings::ServerSettings;
use crate::RikuPaths;

/// File name of the legacy engine choice, directly under the riku root.
pub const ENGINE_FILE: &str = "CONTAINER_ENGINE";

fn legacy_engine(paths: &RikuPaths) -> Result<Option<String>> {
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

/// The engine setting from already loaded `settings`, falling back to the
/// legacy file. For callers that need the settings anyway.
pub fn from_settings(paths: &RikuPaths, settings: &ServerSettings) -> Result<Option<String>> {
    match &settings.engine.program {
        Some(program) => Ok(Some(program.clone())),
        None => legacy_engine(paths),
    }
}

/// Read the engine setting. `None` means the engine is detected. An invalid
/// settings file or an unreadable legacy file is an error.
#[allow(clippy::question_mark)]
pub fn load(paths: &RikuPaths) -> Result<Option<String>> {
    let settings = match crate::settings::load(paths) {
        Ok(settings) => settings,
        Err(e) => return Err(e),
    };
    from_settings(paths, &settings)
}

#[cfg(test)]
#[path = "container_engine_tests.rs"]
mod tests;
