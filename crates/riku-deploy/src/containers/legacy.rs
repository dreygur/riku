//! Recognising the workers the old container plugin left behind: a
//! supervised `compose up` or `docker run` per app.
//!
//! # Security model
//!
//! Worker TOMLs are riku written. A file only counts as an app's legacy
//! worker when its parsed `worker.app` names that app and its command is
//! exactly one of the shapes the plugin printed, so no other worker is ever
//! stopped by a migration.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};

use crate::config::RikuPaths;
use crate::supervisor::config::WorkerConfig;

/// Whether `config` runs one of the two commands the old container plugin
/// printed: `<compose> -f <file> up` (`docker compose`, `podman compose` or
/// `podman-compose`), or `<docker|podman> run --rm -p $PORT:$PORT riku-<app>`.
pub fn is_legacy_worker(config: &WorkerConfig) -> bool {
    let parts: Vec<&str> = config.worker.command.split_whitespace().collect();
    let engine = |name: &str| name == "docker" || name == "podman";
    match parts.as_slice() {
        [rt, "compose", "-f", _, "up"] => engine(rt),
        ["podman-compose", "-f", _, "up"] => true,
        [rt, "run", "--rm", "-p", "$PORT:$PORT", image] => {
            engine(rt) && image.strip_prefix("riku-") == Some(config.worker.app.as_str())
        }
        _ => false,
    }
}

/// Entries of `workers_enabled`; empty when the directory does not exist.
pub(crate) fn enabled_entries(paths: &RikuPaths) -> Result<Vec<PathBuf>> {
    let entries = match fs::read_dir(&paths.workers_enabled) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => {
            return Err(anyhow!(
                "cannot read {}: {e}",
                paths.workers_enabled.display()
            ))
        }
    };
    let mut found = Vec::new();
    for entry in entries {
        match entry {
            Ok(entry) => found.push(entry.path()),
            Err(e) => {
                return Err(anyhow!(
                    "cannot read {}: {e}",
                    paths.workers_enabled.display()
                ))
            }
        }
    }
    found.sort();
    Ok(found)
}

/// Parse the worker config at `path`, following its symlink.
pub(crate) fn read_worker(path: &Path) -> Result<WorkerConfig> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) => return Err(anyhow!("cannot read worker config {}: {e}", path.display())),
    };
    match toml::from_str(&text) {
        Ok(config) => Ok(config),
        Err(e) => Err(anyhow!(
            "cannot parse worker config {}: {e}",
            path.display()
        )),
    }
}

/// Sorted names of apps with at least one enabled legacy worker. Files that
/// cannot be read or parsed are skipped with a warning.
pub fn legacy_apps(paths: &RikuPaths) -> Result<Vec<String>> {
    let entries = match enabled_entries(paths) {
        Ok(entries) => entries,
        Err(e) => return Err(e),
    };
    let mut apps = BTreeSet::new();
    for path in entries {
        if path.extension().and_then(|ext| ext.to_str()) != Some("toml") {
            continue;
        }
        match read_worker(&path) {
            Ok(config) if is_legacy_worker(&config) => {
                apps.insert(config.worker.app);
            }
            Ok(_) => {}
            Err(e) => tracing::warn!("skipping worker config: {e:#}"),
        }
    }
    Ok(apps.into_iter().collect())
}

#[cfg(test)]
#[path = "legacy_tests.rs"]
mod tests;
