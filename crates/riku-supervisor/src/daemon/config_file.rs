//! Reading one worker config file for the supervisor, and the worker its
//! name stands for.
//!
//! The supervisor runs a config only when its file is named
//! `<app>@<kind>.<n>.toml` after the `[worker]` table inside it, so a file
//! name alone tells which process to stop when the file goes away.

use std::fs;
use std::path::Path;

use anyhow::Result;
use riku_util::file_names::worker::{WorkerName, CONFIG_EXTENSION};

use crate::config::WorkerConfig;

/// The worker a config file name stands for, or `None` when the name is not
/// `<app>@<kind>.<n>.toml`.
pub(super) fn named_worker(filename: &str) -> Option<WorkerName> {
    filename
        .strip_suffix(CONFIG_EXTENSION)
        .and_then(|rest| rest.strip_suffix('.'))
        .and_then(WorkerName::parse)
}

fn parse_worker_config(path: &Path) -> Result<WorkerConfig> {
    let config_content = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) => {
            tracing::error!("Error reading config file {}: {}", path.display(), e);
            return Err(e.into());
        }
    };
    match toml::from_str(&config_content) {
        Ok(config) => Ok(config),
        Err(e) => {
            tracing::error!("Error parsing config file {}: {}", path.display(), e);
            Err(e.into())
        }
    }
}

/// Parse a config named after the worker it describes. A misnamed one is
/// logged and yields `Ok(None)`, and is never run.
#[allow(clippy::question_mark)]
pub(super) fn loadable_config(path: &Path, filename: &str) -> Result<Option<WorkerConfig>> {
    let config = match parse_worker_config(path) {
        Ok(config) => config,
        Err(e) => return Err(e),
    };
    let name = config.worker.name();
    if name.file_name() != filename {
        tracing::error!(
            "not running {filename}: the config of worker {name} must be named {}; \
             redeploy '{}' to write it",
            name.file_name(),
            name.app
        );
        return Ok(None);
    }
    Ok(Some(config))
}
