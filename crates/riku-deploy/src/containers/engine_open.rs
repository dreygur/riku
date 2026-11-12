//! Opening the container engine the server is set up to use.
//!
//! # Security model
//!
//! The engine program comes from the operator's `engine.program` setting or a
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

/// What riku says when the engine refused its socket. Adding the deploy user
/// to the docker group is the operator's call, since that group is root on
/// the host, so riku names both ways out and takes neither.
const SOCKET_CHOICE: &str = "The deploy user cannot open the engine's socket. Choose one: \
     add the deploy user to the docker group, which is root-equivalent on this host, \
     or use a rootless engine such as rootless podman (engine.program in settings.toml). \
     riku changes neither the group nor the socket.";

impl EngineUnreachable {
    /// Whether the engine failed because this user may not open its socket.
    /// Older docker says "daemon socket"; docker 29 names only the path,
    /// such as `unix:///var/run/docker.sock`.
    fn socket_refused(&self) -> bool {
        let reason = self.reason.to_lowercase();
        reason.contains("permission denied")
            && (reason.contains("socket") || reason.contains(".sock"))
    }
}

impl std::fmt::Display for EngineUnreachable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.socket_refused() {
            write!(
                f,
                "container engine unreachable: {}. {SOCKET_CHOICE}",
                self.reason
            )
        } else {
            write!(f, "container engine unreachable: {}", self.reason)
        }
    }
}

impl std::error::Error for EngineUnreachable {}

/// Open the engine the server is set up to use.
pub fn open_engine(paths: &RikuPaths) -> Result<Engine> {
    let settings = match riku_config::settings::load(paths) {
        Ok(settings) => settings,
        Err(e) => return Err(anyhow!("invalid server settings: {e}")),
    };
    let search_path = std::env::var_os("PATH").unwrap_or_default();
    let binary = match detect(settings.engine.program.as_deref(), &search_path) {
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
