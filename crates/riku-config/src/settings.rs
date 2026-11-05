//! Server wide settings, read from `<riku root>/settings.toml`.
//!
//! ```toml
//! [engine]
//! call_timeout = "30s"
//! stop_extra   = "30s"
//! pull_timeout = "30m"
//! build_timeout = "30m"
//! ```
//!
//! Every key is optional. The file is operator owned input, so an unknown
//! section or key, a value that is not a string, or a duration that is not
//! a positive integer followed by `s`, `m` or `h` is refused with an error
//! naming the key and the file.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::time::Duration;

use anyhow::{anyhow, Result};

use crate::RikuPaths;

/// File name of the server settings, directly under the riku root.
pub const SETTINGS_FILE: &str = "settings.toml";

/// Time limits for container engine calls.
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
}

impl Default for EngineSettings {
    fn default() -> Self {
        EngineSettings {
            call_timeout: Duration::from_secs(30),
            stop_extra: Duration::from_secs(30),
            pull_timeout: Duration::from_secs(30 * 60),
            build_timeout: Duration::from_secs(30 * 60),
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

#[allow(clippy::question_mark)]
fn engine_settings(table: &toml::Table, file: &Path) -> Result<EngineSettings> {
    let mut engine = EngineSettings::default();
    for (key, value) in table {
        let slot = match key.as_str() {
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
            None => return Err(anyhow!("{}: {section} must be a table", file.display())),
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
