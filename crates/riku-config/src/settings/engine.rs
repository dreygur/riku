//! The `[engine]` section: container engine program and call time limits.

use std::path::Path;
use std::time::Duration;

use anyhow::{anyhow, Result};

/// Container engine settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineSettings {
    /// Engine name such as `podman`, or an absolute path. `None` means the
    /// legacy `CONTAINER_ENGINE` file decides, else the engine is detected.
    /// Validated where it becomes a program to run, not here.
    pub program: Option<String>,
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
            program: None,
            call_timeout: Duration::from_secs(30),
            stop_extra: Duration::from_secs(30),
            pull_timeout: Duration::from_secs(30 * 60),
            build_timeout: Duration::from_secs(30 * 60),
        }
    }
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

/// A non-empty string value at `section.key`, else an error naming both.
pub(super) fn text_at(
    section: &str,
    key: &str,
    value: &toml::Value,
    file: &Path,
) -> Result<String> {
    match value.as_str().map(str::trim) {
        Some(text) if !text.is_empty() => Ok(text.to_string()),
        _ => Err(anyhow!(
            "{}: {section}.{key} must be a non-empty string",
            file.display()
        )),
    }
}

#[allow(clippy::question_mark)]
pub(super) fn engine_settings(table: &toml::Table, file: &Path) -> Result<EngineSettings> {
    let mut engine = EngineSettings::default();
    for (key, value) in table {
        if key == "program" {
            engine.program = match text_at("engine", key, value, file) {
                Ok(text) => Some(text),
                Err(e) => return Err(e),
            };
            continue;
        }
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
