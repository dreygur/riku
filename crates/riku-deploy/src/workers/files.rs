//! Worker config files: writing one into `workers_available`, enabling it by
//! symlink, and removing an app's enabled configs.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::{anyhow, Result};

use super::web_env::is_web_facing;
use crate::config::RikuPaths;
use crate::supervisor::config::{app_worker_configs, create_worker_config, WorkerConfig};
use crate::util::file_names::worker::WorkerName;

/// Whether `path` is named after a copy of a web facing kind.
fn is_web_copy(path: &Path) -> bool {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .and_then(WorkerName::parse)
        .is_some_and(|name| is_web_facing(&name.kind))
}

/// Remove existing worker symlinks from `workers_enabled` to trigger a restart.
///
/// Web facing kinds are left alone: the deploy replaces their copies one at
/// a time instead.
pub(super) fn remove_stale_configs(app: &str, paths: &RikuPaths) {
    let stale = app_worker_configs(&paths.workers_enabled, app)
        .into_iter()
        .filter(|entry| !is_web_copy(entry));
    for entry in stale {
        if let Err(e) = fs::remove_file(&entry) {
            tracing::warn!("Could not remove stale worker config {:?}: {}", entry, e);
        }
    }
}

/// A worker config built and serialized but not yet on disk.
pub(super) struct RenderedConfig {
    pub config: WorkerConfig,
    text: String,
}

/// Build and serialize one worker config without touching the disk, so every
/// fallible step but the write itself happens here.
pub(super) fn render_worker_config(
    app: &str,
    app_path: &Path,
    kind: &str,
    command: &str,
    index: u32,
    env: HashMap<String, String>,
    paths: &RikuPaths,
) -> Result<RenderedConfig> {
    let log_path = paths
        .log_root
        .join(app)
        .join(format!("{}.{}.log", kind, index));

    let config = create_worker_config(
        app,
        kind,
        command,
        index,
        env,
        &app_path.to_string_lossy(),
        &log_path.to_string_lossy(),
    );
    match toml::to_string(&config) {
        Ok(text) => Ok(RenderedConfig { config, text }),
        Err(e) => Err(anyhow!(
            "cannot serialize worker config {}: {e}",
            config.worker.name().file_name()
        )),
    }
}

/// Write `rendered` to `workers_available` and symlink it into
/// `workers_enabled`.
pub(super) fn install_worker_config(rendered: &RenderedConfig, paths: &RikuPaths) -> Result<()> {
    let filename = rendered.config.worker.name().file_name();
    let available = paths.workers_available.join(&filename);
    let enabled = paths.workers_enabled.join(&filename);

    // The config carries the app's whole environment, secrets included.
    if let Err(e) = crate::util::write_private(&available, rendered.text.as_bytes()) {
        return Err(anyhow!("cannot write {}: {e}", available.display()));
    }
    if let Err(e) = crate::util::fs::replace_symlink(&available, &enabled) {
        return Err(anyhow!("cannot enable {filename}: {e}"));
    }

    tracing::debug!("created worker config {filename}");
    Ok(())
}

/// Write a single worker TOML config to `workers_available`, symlink it into
/// `workers_enabled`, and return the config written.
pub(super) fn write_worker_config(
    app: &str,
    app_path: &Path,
    kind: &str,
    command: &str,
    index: u32,
    env: HashMap<String, String>,
    paths: &RikuPaths,
) -> Result<WorkerConfig> {
    let rendered = match render_worker_config(app, app_path, kind, command, index, env, paths) {
        Ok(rendered) => rendered,
        Err(e) => return Err(e),
    };
    match install_worker_config(&rendered, paths) {
        Ok(()) => Ok(rendered.config),
        Err(e) => Err(e),
    }
}
