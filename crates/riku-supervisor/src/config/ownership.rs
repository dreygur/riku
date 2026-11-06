//! Which worker config files in `workers-available/` or `workers-enabled/`
//! belong to an app.
//!
//! Every TOML config riku writes records its app under `[worker] app`, and
//! that field decides.
//!
//! A file with no readable owner (a `.ini` file, an unparsable TOML, a
//! dangling symlink) falls back to its name, and only to the
//! `<app>@<kind>.<n>` form (see [`riku_util::file_names::worker`]), which
//! names one app. Any other name is claimed by no app, because callers
//! delete what this returns and leaving a stray file is the safe failure.

use riku_util::file_names::worker::WorkerName;
use serde::Deserialize;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

#[cfg(test)]
#[path = "ownership_tests.rs"]
mod tests;

const CONFIG_EXTENSIONS: [&str; 2] = ["toml", "ini"];

#[derive(Deserialize)]
struct OwnerSection {
    worker: OwnerField,
}

#[derive(Deserialize)]
struct OwnerField {
    app: String,
}

/// The `.toml` and `.ini` worker configs in `dir` that belong to `app`,
/// sorted by path. A missing `dir` yields an empty list.
pub fn app_worker_configs(dir: &Path, app: &str) -> Vec<PathBuf> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == ErrorKind::NotFound => return Vec::new(),
        Err(e) => {
            tracing::warn!("cannot list worker configs in {}: {e}", dir.display());
            return Vec::new();
        }
    };
    let mut configs: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| is_config_file(path) && belongs_to(path, app))
        .collect();
    configs.sort();
    configs
}

fn is_config_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| CONFIG_EXTENSIONS.contains(&ext))
}

fn belongs_to(path: &Path, app: &str) -> bool {
    match recorded_owner(path) {
        Some(owner) => owner == app,
        None => named_for(path, app),
    }
}

fn recorded_owner(path: &Path) -> Option<String> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(_) => return None,
    };
    match toml::from_str::<OwnerSection>(&text) {
        Ok(section) => Some(section.worker.app),
        Err(_) => None,
    }
}

fn named_for(path: &Path, app: &str) -> bool {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .and_then(WorkerName::parse)
        .is_some_and(|name| name.app == app)
}
