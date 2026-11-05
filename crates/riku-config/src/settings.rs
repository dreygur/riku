//! Server wide settings, read from `<riku root>/settings.toml`.
//!
//! ```toml
//! [engine]
//! call_timeout = "30s"
//! stop_extra   = "30s"
//! pull_timeout = "30m"
//! build_timeout = "30m"
//! mode = "rootless"
//! host = "unix:///run/user/1000/docker.sock"
//! ```
//!
//! Every key is optional. The file is operator owned input, so an unknown
//! section or key, a value that is not a string, a duration that is not
//! a positive integer followed by `s`, `m` or `h`, a `mode` other than
//! `rootless` or `rootful`, and an empty `host` or one holding control
//! characters are all refused with an error naming the key and the file.
//! `host` becomes the engine client's `DOCKER_HOST`; it never reaches a
//! shell or a path.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::time::Duration;

use anyhow::{anyhow, Result};

use crate::RikuPaths;

/// File name of the server settings, directly under the riku root.
pub const SETTINGS_FILE: &str = "settings.toml";

/// Which engine daemon riku talks to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EngineMode {
    /// A daemon running as the deploy user, reached through its own runtime
    /// directory.
    #[default]
    Rootless,
    /// A system wide daemon reached through the address in `host`.
    Rootful,
}

/// Time limits and connection settings for container engine calls.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineSettings {
    /// Limit for a single quick engine call such as `ps`, `inspect` or `rm`.
    pub call_timeout: Duration,
    /// Time allowed for `stop` beyond the container's own grace period.
    pub stop_extra: Duration,
    /// Limit for a whole image pull.
    pub pull_timeout: Duration,
    /// Limit for a whole image build.
    pub build_timeout: Duration,
    /// Rootless unless `settings.toml` says otherwise.
    pub mode: EngineMode,
    /// Engine address passed as `DOCKER_HOST`. `None` leaves the engine
    /// client on its own default socket.
    pub host: Option<String>,
}

impl Default for EngineSettings {
    fn default() -> Self {
        EngineSettings {
            call_timeout: Duration::from_secs(30),
            stop_extra: Duration::from_secs(30),
            pull_timeout: Duration::from_secs(30 * 60),
            build_timeout: Duration::from_secs(30 * 60),
            mode: EngineMode::default(),
            host: None,
        }
    }
}

/// Everything in `settings.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ServerSettings {
    pub engine: EngineSettings,
}

fn parse_duration(text: &str) -> Option<Duration> {
    let (digits, multiplier) = match text.split_at_checked(text.len().saturating_sub(1)) {
        Some((digits, "s")) => (digits, 1),
        Some((digits, "m")) => (digits, 60),
        Some((digits, "h")) => (digits, 3600),
        _ => return None,
    };
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    match digits.parse::<u64>() {
        Ok(count) if count > 0 => count.checked_mul(multiplier).map(Duration::from_secs),
        _ => None,
    }
}

fn duration_at(key: &str, value: &toml::Value, file: &Path) -> Result<Duration> {
    let parsed = value.as_str().and_then(parse_duration);
    match parsed {
        Some(duration) => Ok(duration),
        None => Err(anyhow!(
            "{}: engine.{key} must be a positive whole number followed by s, m or h, such as \"30s\"",
            file.display()
        )),
    }
}

fn mode_at(value: &toml::Value, file: &Path) -> Result<EngineMode> {
    match value.as_str() {
        Some("rootless") => Ok(EngineMode::Rootless),
        Some("rootful") => Ok(EngineMode::Rootful),
        _ => Err(anyhow!(
            "{}: engine.mode must be \"rootless\" or \"rootful\"",
            file.display()
        )),
    }
}

fn host_at(value: &toml::Value, file: &Path) -> Result<String> {
    match value.as_str() {
        Some(host) if !host.is_empty() && !host.chars().any(char::is_control) => {
            Ok(host.to_string())
        }
        _ => Err(anyhow!(
            "{}: engine.host must be a non empty string without control characters",
            file.display()
        )),
    }
}

#[allow(clippy::question_mark)]
fn engine_settings(table: &toml::Table, file: &Path) -> Result<EngineSettings> {
    let mut engine = EngineSettings::default();
    for (key, value) in table {
        let slot = match key.as_str() {
            "mode" => {
                engine.mode = match mode_at(value, file) {
                    Ok(mode) => mode,
                    Err(e) => return Err(e),
                };
                continue;
            }
            "host" => {
                engine.host = match host_at(value, file) {
                    Ok(host) => Some(host),
                    Err(e) => return Err(e),
                };
                continue;
            }
            "call_timeout" => &mut engine.call_timeout,
            "stop_extra" => &mut engine.stop_extra,
            "pull_timeout" => &mut engine.pull_timeout,
            "build_timeout" => &mut engine.build_timeout,
            _ => return Err(anyhow!("{}: unknown key engine.{key}", file.display())),
        };
        *slot = match duration_at(key, value, file) {
            Ok(duration) => duration,
            Err(e) => return Err(e),
        };
    }
    Ok(engine)
}

#[allow(clippy::question_mark)]
fn parse_settings(raw: &str, file: &Path) -> Result<ServerSettings> {
    let root: toml::Table = match raw.parse() {
        Ok(root) => root,
        Err(e) => return Err(anyhow!("{}: invalid TOML: {e}", file.display())),
    };
    let mut settings = ServerSettings::default();
    for (section, value) in &root {
        if section != "engine" {
            return Err(anyhow!("{}: unknown section [{section}]", file.display()));
        }
        let table = match value.as_table() {
            Some(table) => table,
            None => return Err(anyhow!("{}: engine must be a table", file.display())),
        };
        settings.engine = match engine_settings(table, file) {
            Ok(engine) => engine,
            Err(e) => return Err(e),
        };
    }
    Ok(settings)
}

/// Read the settings file. A missing file gives the defaults; any other read
/// failure, or an invalid file, is an error.
pub fn load(paths: &RikuPaths) -> Result<ServerSettings> {
    match fs::read_to_string(&paths.settings_file) {
        Ok(raw) => parse_settings(&raw, &paths.settings_file),
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
