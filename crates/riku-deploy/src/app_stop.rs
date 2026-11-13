//! Stopping every worker of a Procfile app, as `riku stop` does.
//!
//! The supervisor stops a worker once its `workers-enabled` link is gone.
//! The copy's `workers-available` config and its own uWSGI socket are removed
//! too, as a scale-down does, so neither lingers until the app is destroyed.

// Project rule 16 forbids `?`, and this lint asks for exactly that rewrite.
#![allow(clippy::question_mark)]

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use riku_container::types::CopyIndex;
use riku_supervisor::config::app_worker_configs;

use crate::config::RikuPaths;
use crate::util::backend::Backend;

fn remove_if_present(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
        Err(e) => Err(anyhow!("cannot remove {}: {e}", path.display())),
    }
}

/// The socket riku made for the copy `config` describes, when its config
/// names exactly that socket.
fn own_socket(app: &str, config: &Path, paths: &RikuPaths) -> Option<PathBuf> {
    let worker = match crate::router::parsed_worker(config) {
        Some(worker) => worker,
        None => return None,
    };
    let copy = match CopyIndex::new(worker.worker.ordinal) {
        Some(copy) => copy,
        None => return None,
    };
    let own = crate::router::copy_socket(paths, app, copy);
    match crate::router::worker_backend(&worker) {
        Ok(Backend::Unix(socket)) if socket == own => Some(own),
        _ => None,
    }
}

/// Stop every worker of `app`. Returns how many were enabled, so the caller
/// can tell an app that was not running.
pub fn stop_procfile_app(app: &str, paths: &RikuPaths) -> Result<usize> {
    let enabled = app_worker_configs(&paths.workers_enabled, app);
    let available = app_worker_configs(&paths.workers_available, app);
    let sockets: Vec<PathBuf> = available
        .iter()
        .filter_map(|config| own_socket(app, config, paths))
        .collect();

    for file in enabled.iter().chain(&sockets).chain(&available) {
        if let Err(e) = remove_if_present(file) {
            return Err(e);
        }
    }
    Ok(enabled.len())
}

#[cfg(test)]
#[path = "app_stop_tests.rs"]
mod tests;
