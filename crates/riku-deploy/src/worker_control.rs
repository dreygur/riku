//! Per-instance worker control: restart or remove a single worker ordinal
//! without touching its siblings.
//!
//! Security: `kind` and `ordinal` arrive from the dashboard's control plane
//! (network input, not the trusted Procfile-parsing path), so both are
//! validated before being interpolated into a filesystem path.

use anyhow::{bail, Result};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::app_restart::restart_configs;
use crate::config::RikuPaths;
use crate::scaling::apply_scaling_deltas;
use crate::util::file_names::worker::WorkerName;
use crate::util::procfile::is_valid_kind;
use crate::util::validate_app_name;
use crate::workers::read_scaling_count;

/// A worker `kind` must be a Procfile process type before it is used to
/// build a path.
fn validate_kind(kind: &str) -> Result<()> {
    if !is_valid_kind(kind) {
        bail!("invalid worker kind '{kind}'");
    }
    Ok(())
}

/// The worker named by an unvalidated app, kind and ordinal.
#[allow(clippy::question_mark)]
fn checked_worker(app: &str, kind: &str, ordinal: u32) -> Result<WorkerName> {
    let app = match validate_app_name(app) {
        Ok(app) => app,
        Err(e) => return Err(e),
    };
    if let Err(e) = validate_kind(kind) {
        return Err(e);
    }
    Ok(WorkerName::new(&app, kind, ordinal))
}

#[allow(clippy::question_mark)]
fn worker_paths(
    app: &str,
    paths: &RikuPaths,
    kind: &str,
    ordinal: u32,
) -> Result<(PathBuf, PathBuf)> {
    let filename = match checked_worker(app, kind, ordinal) {
        Ok(name) => name.file_name(),
        Err(e) => return Err(e),
    };
    Ok((
        paths.workers_available.join(&filename),
        paths.workers_enabled.join(&filename),
    ))
}

/// Restart exactly one worker the way `riku restart` restarts an app: its
/// config's mtime advances and the supervisor gets a SIGHUP. The supervisor
/// compares the mtime of the file the enabled link points at, so moving the
/// link alone would not restart anything.
#[allow(clippy::question_mark)]
pub fn restart_worker(app: &str, paths: &RikuPaths, kind: &str, ordinal: u32) -> Result<()> {
    let (available, enabled) = match worker_paths(app, paths, kind, ordinal) {
        Ok(found) => found,
        Err(e) => return Err(e),
    };
    if !available.exists() || !enabled.exists() {
        bail!(
            "no running worker '{app}' {kind}.{ordinal}; it may have been scaled away or stopped"
        );
    }
    restart_configs(&[enabled], paths)
}

/// Remove exactly one worker's config, stopping it without touching sibling
/// ordinals.
///
/// Only the *highest* ordinal for `kind` can be deleted this way, and doing
/// so goes through the same `apply_scaling_deltas` path the "scale down"
/// button uses (delta -1) rather than just unlinking the file. Riku's
/// scaling model is "N contiguous workers, 1..=N", there's no such thing as
/// a sparse hole at ordinal 2 while 1 and 3 exist: so deleting a
/// non-highest ordinal directly would leave the SCALING file still saying
/// the old count, and the very next deploy/config-set would silently
/// recreate exactly the instance the operator just removed. Restart a
/// non-highest crashed ordinal instead (see `restart_worker`); to remove it
/// permanently, scale down to it first.
#[allow(clippy::question_mark)]
pub fn delete_worker(app: &str, paths: &RikuPaths, kind: &str, ordinal: u32) -> Result<()> {
    let app = match checked_worker(app, kind, ordinal) {
        Ok(name) => name.app,
        Err(e) => return Err(e),
    };

    let current = match read_scaling_count(paths, &app, kind) {
        Ok(current) => current,
        Err(e) => return Err(e),
    };
    if ordinal != current {
        bail!(
            "'{app}' {kind}.{ordinal} isn't the highest instance (currently {kind}.{current}) -- \
             riku only supports scaling down from the top; restart {kind}.{ordinal} instead, \
             or scale {kind} down to {ordinal} first"
        );
    }

    let mut deltas = HashMap::new();
    deltas.insert(kind.to_string(), -1i64);
    let mut workers = HashMap::new();
    workers.insert(kind.to_string(), String::new());
    apply_scaling_deltas(&app, paths, &deltas, &workers).map(|_| ())
}

#[cfg(test)]
#[path = "worker_control_tests.rs"]
mod tests;
