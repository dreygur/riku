//! Listing and parsing the worker configs in `workers_enabled`.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};

use crate::config::RikuPaths;
use crate::supervisor::config::WorkerConfig;

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
