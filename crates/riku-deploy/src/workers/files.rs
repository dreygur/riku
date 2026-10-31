//! Worker config files: writing one into `workers_available`, enabling it by
//! symlink, and removing an app's enabled configs.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::Result;

use crate::config::RikuPaths;
use crate::util::echo;

/// Remove existing worker symlinks from `workers_enabled` to trigger a restart.
///
/// Uses `"{app}-*"` not `"{app}*"` to avoid touching configs for apps
/// whose names share a prefix (e.g. "foo" would otherwise match "foobar").
pub(super) fn remove_stale_configs(app: &str, paths: &RikuPaths) {
    for ext in &["toml", "ini"] {
        let pattern = paths.workers_enabled.join(format!("{}-*.{}", app, ext));
        if let Ok(entries) = glob::glob(pattern.to_str().unwrap_or("")) {
            for entry in entries.flatten() {
                if let Err(e) = fs::remove_file(&entry) {
                    tracing::warn!("Could not remove stale worker config {:?}: {}", entry, e);
                }
            }
        }
    }
}

/// Write a single worker TOML config to `workers_available` and symlink it into `workers_enabled`.
pub(super) fn write_worker_config(
    app: &str,
    app_path: &Path,
    kind: &str,
    command: &str,
    index: u32,
    env: HashMap<String, String>,
    paths: &RikuPaths,
) -> Result<()> {
    use crate::supervisor::config::create_worker_config;

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

    crate::util::write_atomic(&available, toml::to_string(&config)?.as_bytes())?;

    // Atomic symlink swap: create the new symlink under a temp name, then
    // `rename` it over `enabled`. A crash or concurrent reader between a
    // `remove_file` and a fresh `symlink` would otherwise see `enabled`
    // briefly missing entirely; `rename` guarantees the path always
    // resolves to either the old or the new symlink, never nothing.
    let tmp_link = paths
        .workers_enabled
        .join(format!(".{}.tmp-{}", filename, std::process::id()));
    let _ = fs::remove_file(&tmp_link);
    std::os::unix::fs::symlink(&available, &tmp_link)?;
    fs::rename(&tmp_link, &enabled)?;

    echo(
        &format!("-----> Created worker config: {}", filename),
        "green",
    );
    Ok(())
}
