//! Generic worker configuration creation for deployed apps.
//!
//! Handles Procfile parsing and worker config generation.
//! Scaling delta logic lives in `super::scaling`.

use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::config::RikuPaths;
use crate::util::echo;

pub(crate) use super::scaling::apply_scaling_deltas;

mod web_env;

use web_env::build_worker_env;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Read the scaling count for a given process kind from the SCALING file.
///
/// Returns 1 if the file doesn't exist or the kind isn't listed.
pub fn read_scaling_count(paths: &RikuPaths, app: &str, kind: &str) -> Result<u32> {
    let scaling_path = paths.env_root.join(app).join("SCALING");
    if !scaling_path.exists() {
        return Ok(1);
    }
    let content = fs::read_to_string(&scaling_path)?;
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(pos) = line.find('=') {
            let key = line[..pos].trim();
            let val = line[pos + 1..].trim();
            if key == kind {
                if let Ok(n) = val.parse::<u32>() {
                    return Ok(n);
                }
            }
        }
    }
    Ok(1)
}

/// Create worker configs for every process entry in the app's Procfile.
///
/// `start_cmd` is an optional fallback command supplied by the runtime plugin
/// via its `start` subcommand. It is used only when a Procfile entry has an
/// empty command.
#[allow(clippy::question_mark)]
pub fn create_workers_generic(
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
    start_cmd: Option<&str>,
) -> Result<()> {
    if should_restart(env) {
        remove_stale_configs(app, paths);
    }

    let entries = match parse_procfile(app_path) {
        Ok(Some(e)) => e,
        Ok(None) => return Ok(()),
        Err(e) => return Err(e),
    };

    for (kind, command) in &entries {
        let effective_cmd = if command.is_empty() {
            start_cmd.unwrap_or(command.as_str())
        } else {
            command.as_str()
        };
        let count = match read_scaling_count(paths, app, kind) {
            Ok(count) => count,
            Err(e) => return Err(e),
        };
        for i in 1..=count {
            let worker_env = match build_worker_env(app, kind, env, paths) {
                Ok(worker_env) => worker_env,
                Err(e) => return Err(e),
            };
            if let Err(e) =
                write_worker_config(app, app_path, kind, effective_cmd, i, worker_env, paths)
            {
                return Err(e);
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Returns true when `RIKU_AUTO_RESTART` is not explicitly disabled.
fn should_restart(env: &HashMap<String, String>) -> bool {
    env.get("RIKU_AUTO_RESTART")
        .map(|v| v.to_lowercase() != "false" && v != "0" && v != "no")
        .unwrap_or(true)
}

/// Remove existing worker symlinks from `workers_enabled` to trigger a restart.
///
/// Uses `"{app}-*"` not `"{app}*"` to avoid touching configs for apps
/// whose names share a prefix (e.g. "foo" would otherwise match "foobar").
fn remove_stale_configs(app: &str, paths: &RikuPaths) {
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

/// Parse the Procfile at `app_path/Procfile` into `(kind, command)` pairs.
///
/// Returns `None` (and prints a warning) when no Procfile is found.
/// Comment lines and blank lines are skipped.
fn parse_procfile(app_path: &Path) -> Result<Option<Vec<(String, String)>>> {
    let procfile_path = app_path.join("Procfile");
    if !procfile_path.exists() {
        echo(
            "-----> No Procfile found, skipping process creation",
            "yellow",
        );
        return Ok(None);
    }

    let content = fs::read_to_string(&procfile_path)?;
    let entries = content.lines().filter_map(parse_procfile_line).collect();

    Ok(Some(entries))
}

/// Parse a single Procfile line into `(kind, command)`, or `None` for blank/comment lines.
fn parse_procfile_line(line: &str) -> Option<(String, String)> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let pos = line.find(':')?;
    let kind = line[..pos].trim().to_string();
    let command = line[pos + 1..].trim().to_string();
    Some((kind, command))
}

/// Write a single worker TOML config to `workers_available` and symlink it into `workers_enabled`.
fn write_worker_config(
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

#[cfg(test)]
#[path = "workers_tests.rs"]
mod tests;
