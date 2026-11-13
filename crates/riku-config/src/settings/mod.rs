//! Server wide settings, read from `<riku root>/settings.toml`.
//!
//! ```toml
//! [router]
//! name = "caddy"
//!
//! [engine]
//! program = "podman"
//! call_timeout = "30s"
//! stop_extra   = "30s"
//! pull_timeout = "30m"
//! build_timeout = "30m"
//! ```
//!
//! Every key is optional. The file is operator owned input, so an unknown
//! section or key, a value of the wrong type, and a duration that is not a
//! positive integer followed by `s`, `m` or `h` are all refused with an error
//! naming the key and the file. Nothing here reaches a shell or a path; the
//! engine program is validated where it becomes a program to run.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use anyhow::{anyhow, Result};

use crate::RikuPaths;

mod engine;
mod routing;
mod save;

pub use engine::EngineSettings;
pub use routing::{routing_section, FallbackPattern, PortRange, RoutingSettings};
pub use save::{add_routing_section, save_key};

/// File name of the server settings, directly under the riku root.
pub const SETTINGS_FILE: &str = "settings.toml";

/// Everything in `settings.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ServerSettings {
    /// `[router] name`. `None` means nginx.
    pub router: Option<String>,
    pub engine: EngineSettings,
    pub routing: RoutingSettings,
}

#[allow(clippy::question_mark)]
fn router_name(table: &toml::Table, file: &Path) -> Result<Option<String>> {
    let mut name = None;
    for (key, value) in table {
        if key != "name" {
            return Err(anyhow!("{}: unknown key router.{key}", file.display()));
        }
        name = match engine::text_at("router", key, value, file) {
            Ok(text) => Some(text),
            Err(e) => return Err(e),
        };
    }
    Ok(name)
}

/// Parse settings text. `file` is only used in error messages.
#[allow(clippy::question_mark)]
pub fn parse(raw: &str, file: &Path) -> Result<ServerSettings> {
    let root: toml::Table = match raw.parse() {
        Ok(root) => root,
        Err(e) => return Err(anyhow!("{}: invalid TOML: {e}", file.display())),
    };
    let mut settings = ServerSettings::default();
    for (section, value) in &root {
        let table = match value.as_table() {
            Some(table) => table,
            None => return Err(anyhow!("{}: {section} must be a table", file.display())),
        };
        match section.as_str() {
            "engine" => {
                settings.engine = match engine::engine_settings(table, file) {
                    Ok(engine) => engine,
                    Err(e) => return Err(e),
                };
            }
            "router" => {
                settings.router = match router_name(table, file) {
                    Ok(name) => name,
                    Err(e) => return Err(e),
                };
            }
            "routing" => {
                settings.routing = match routing::routing_settings(table, file) {
                    Ok(routing) => routing,
                    Err(e) => return Err(e),
                };
            }
            _ => return Err(anyhow!("{}: unknown section [{section}]", file.display())),
        }
    }
    Ok(settings)
}

/// Read the settings file. A missing file gives the defaults; any other read
/// failure, or an invalid file, is an error.
pub fn load(paths: &RikuPaths) -> Result<ServerSettings> {
    match fs::read_to_string(&paths.settings_file) {
        Ok(raw) => parse(&raw, &paths.settings_file),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(ServerSettings::default()),
        Err(e) => Err(anyhow!(
            "failed to read {}: {e}",
            paths.settings_file.display()
        )),
    }
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
