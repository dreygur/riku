//! Whether the engine reached is running rootless.
//!
//! Security model: the answer is trusted only as the engine's own report from
//! `info --format json`, parsed as data. Output in neither known shape is an
//! error, and in rootless mode that error and a rootful engine are both
//! refused, never used.

use std::ffi::OsString;

use anyhow::{anyhow, Result};
use serde_json::Value;

use super::Engine;

const DOCKER_ROOTLESS_OPTION: &str = "name=rootless";
const SOCKET_VARIABLE: &str = "DOCKER_HOST";

/// Docker lists `name=rootless` among `SecurityOptions`, each entry possibly
/// followed by comma separated settings. Podman reports `host.security.rootless`.
fn parse_rootless(info_json: &str) -> Result<bool> {
    let doc: Value = match serde_json::from_str(info_json) {
        Ok(doc) => doc,
        Err(e) => return Err(anyhow!("engine info output is not JSON: {e}")),
    };
    if let Some(options) = doc["SecurityOptions"].as_array() {
        return Ok(options.iter().filter_map(Value::as_str).any(|option| {
            option
                .split(',')
                .any(|setting| setting == DOCKER_ROOTLESS_OPTION)
        }));
    }
    match doc["host"]["security"]["rootless"].as_bool() {
        Some(rootless) => Ok(rootless),
        None => Err(anyhow!(
            "engine info reports neither SecurityOptions nor host.security.rootless"
        )),
    }
}

impl Engine {
    /// Ask the engine whether it runs rootless.
    pub fn is_rootless(&self) -> Result<bool> {
        let args = [
            OsString::from("info"),
            OsString::from("--format"),
            OsString::from("json"),
        ];
        let json = match self.output(&args) {
            Ok(json) => json,
            Err(e) => return Err(anyhow!("cannot read the engine info: {e}")),
        };
        parse_rootless(&json)
    }

    /// Refuse the engine unless it reports rootless, for a server in rootless
    /// mode. The error names the socket the client reached.
    pub fn require_rootless(&self) -> Result<()> {
        let rootless = match self.is_rootless() {
            Ok(rootless) => rootless,
            Err(e) => return Err(anyhow!("rootless mode needs a rootless engine: {e}")),
        };
        if rootless {
            return Ok(());
        }
        let socket = self
            .env()
            .iter()
            .find(|(name, _)| name == SOCKET_VARIABLE)
            .map_or("its default socket", |(_, value)| value.as_str());
        Err(anyhow!(
            "engine mode is rootless but {} reached a rootful engine at {socket}; \
             run `sudo riku init` to set up a rootless engine",
            self.program().display()
        ))
    }
}

#[cfg(test)]
#[path = "info_tests.rs"]
mod tests;
