//! Opening the container engine the server is set up to use.
//!
//! # Security model
//!
//! The engine program comes from the operator's `CONTAINER_ENGINE` setting or a
//! fixed list searched on `PATH`, never from app input. `settings.toml` is
//! operator owned input refused when invalid.

use anyhow::{anyhow, Result};
use riku_container::engine::{detect, Engine};

use crate::config::RikuPaths;

/// A container engine cannot be detected, executed, or does not answer probes.
#[derive(Debug)]
pub struct EngineUnreachable {
    reason: String,
}

impl EngineUnreachable {
    pub(crate) fn new(reason: String) -> Self {
        Self { reason }
    }
}

impl std::fmt::Display for EngineUnreachable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "container engine unreachable: {}", self.reason)
    }
}

impl std::error::Error for EngineUnreachable {}

/// Open the engine the server is set up to use.
pub fn open_engine(paths: &RikuPaths) -> Result<Engine> {
    let settings = match riku_config::settings::load(paths) {
        Ok(settings) => settings,
        Err(e) => return Err(anyhow!("invalid server settings: {e}")),
    };
    let setting = match riku_config::container_engine::load(paths) {
        Ok(setting) => setting,
        Err(e) => return Err(e),
    };
    let search_path = std::env::var_os("PATH").unwrap_or_default();
    let binary = match detect(setting.as_deref(), &search_path) {
        Ok(binary) => binary,
        Err(e) => return Err(anyhow::Error::new(EngineUnreachable::new(e.to_string()))),
    };
    let engine = match Engine::open(binary) {
        Ok(engine) => engine,
        Err(e) => return Err(anyhow::Error::new(EngineUnreachable::new(e.to_string()))),
    };
    Ok(engine.with_settings(settings.engine))
}

/// Reclassify an operation error as [`EngineUnreachable`] if the engine daemon
/// stopped answering after the engine was opened.
pub(super) fn reclassify<T>(engine: &Engine, outcome: Result<T>) -> Result<T> {
    match outcome {
        Ok(value) => Ok(value),
        Err(e) => match engine.ping() {
            Ok(()) => Err(e),
            Err(ping_error) => Err(anyhow::Error::new(EngineUnreachable::new(format!(
                "{ping_error}; original failure: {e:#}"
            )))),
        },
    }
}

#[cfg(test)]
#[path = "engine_open_tests.rs"]
mod tests;
