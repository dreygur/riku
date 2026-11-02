//! Worker config files: writing one into `workers_available`, enabling it by
//! symlink, and removing an app's enabled configs.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::{anyhow, Result};

use super::web_env::is_web_facing;
use crate::config::RikuPaths;
use crate::supervisor::config::{create_worker_config, WorkerConfig};
use crate::util::echo;

/// The kind in a `<app>-<kind>-<n>.<ext>` file name, taken from the right
/// because app names contain hyphens.
fn file_kind(path: &Path) -> Option<&str> {
    let stem = match path.file_stem().and_then(|stem| stem.to_str()) {
        Some(stem) => stem,
        None => return None,
    };
    match stem.rsplit_once('-') {
        Some((rest, _ordinal)) => rest.rsplit_once('-').map(|(_, kind)| kind),
        None => None,
    }
}

/// Remove existing worker symlinks from `workers_enabled` to trigger a restart.
///
/// Web facing kinds are left alone: the deploy replaces their copies one at
/// a time instead.
///
/// Uses `"{app}-*"` not `"{app}*"` to avoid touching configs for apps
/// whose names share a prefix (e.g. "foo" would otherwise match "foobar").
pub(super) fn remove_stale_configs(app: &str, paths: &RikuPaths) {
    for ext in &["toml", "ini"] {
        let pattern = paths.workers_enabled.join(format!("{}-*.{}", app, ext));
        if let Ok(entries) = glob::glob(pattern.to_str().unwrap_or("")) {
            let stale = entries
                .flatten()
                .filter(|entry| !file_kind(entry).is_some_and(is_web_facing));
            for entry in stale {
                if let Err(e) = fs::remove_file(&entry) {
                    tracing::warn!("Could not remove stale worker config {:?}: {}", entry, e);
                }
            }
        }
    }
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

    let filename = format!("{}-{}-{}.toml", app, kind, index);
    let available = paths.workers_available.join(&filename);
    let enabled = paths.workers_enabled.join(&filename);

    let serialized = match toml::to_string(&config) {
        Ok(text) => text,
        Err(e) => return Err(anyhow!("cannot serialize worker config {filename}: {e}")),
    };
    if let Err(e) = crate::util::write_atomic(&available, serialized.as_bytes()) {
        return Err(anyhow!("cannot write {}: {e}", available.display()));
    }

    // Atomic symlink swap: create the new symlink under a temp name, then
    // `rename` it over `enabled`. A crash or concurrent reader between a
    // `remove_file` and a fresh `symlink` would otherwise see `enabled`
    // briefly missing entirely; `rename` guarantees the path always
    // resolves to either the old or the new symlink, never nothing.
    let tmp_link = paths
        .workers_enabled
        .join(format!(".{}.tmp-{}", filename, std::process::id()));
    let _ = fs::remove_file(&tmp_link);
    if let Err(e) = std::os::unix::fs::symlink(&available, &tmp_link) {
        return Err(anyhow!("cannot link {}: {e}", tmp_link.display()));
    }
    if let Err(e) = fs::rename(&tmp_link, &enabled) {
        return Err(anyhow!("cannot enable {}: {e}", enabled.display()));
    }

    echo(
        &format!("-----> Created worker config: {}", filename),
        "green",
    );
    Ok(config)
}
