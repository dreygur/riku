//! Per-instance worker control: restart or remove a single worker ordinal
//! without touching its siblings.
//!
//! Security: `kind` and `ordinal` arrive from the dashboard's control plane
//! (network input, not the trusted Procfile-parsing path), so both are
//! validated before being interpolated into a filesystem path.

use anyhow::{bail, Result};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::config::RikuPaths;
use crate::scaling::apply_scaling_deltas;
use crate::util::validate_app_name;
use crate::workers::read_scaling_count;

/// Worker `kind` is always a short Procfile process-type token (`web`,
/// `worker`, `cron0`, ...) -- reject anything that isn't exactly that shape
/// before it's used to build a path.
fn validate_kind(kind: &str) -> Result<()> {
    if kind.is_empty() || !kind.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        bail!("invalid worker kind '{kind}'");
    }
    Ok(())
}

fn worker_paths(
    app: &str,
    paths: &RikuPaths,
    kind: &str,
    ordinal: u32,
) -> Result<(PathBuf, PathBuf)> {
    let app = validate_app_name(app)?;
    validate_kind(kind)?;
    let filename = format!("{app}-{kind}-{ordinal}.toml");
    Ok((
        paths.workers_available.join(&filename),
        paths.workers_enabled.join(&filename),
    ))
}

/// Restart exactly one worker by re-triggering its symlink swap in
/// `workers-enabled/` — the same atomic rename dance `write_worker_config`
/// performs on every deploy, which the supervisor's file watcher already
/// reliably treats as "this worker changed, stop and respawn it" (see
/// `handle_modified_config`). Content is unchanged; only the symlink moves,
/// which is what triggers the watcher.
pub fn restart_worker(app: &str, paths: &RikuPaths, kind: &str, ordinal: u32) -> Result<()> {
    let (available, enabled) = worker_paths(app, paths, kind, ordinal)?;
    if !available.exists() {
        bail!(
            "no worker config for '{app}' {kind}.{ordinal} -- it may have been scaled away; nothing to restart"
        );
    }

    let tmp_link = paths.workers_enabled.join(format!(
        ".{}-{}-{}.toml.tmp-{}",
        app,
        kind,
        ordinal,
        std::process::id()
    ));
    let _ = fs::remove_file(&tmp_link);
    std::os::unix::fs::symlink(&available, &tmp_link)?;
    fs::rename(&tmp_link, &enabled)?;
    Ok(())
}

/// Remove exactly one worker's config, stopping it without touching sibling
/// ordinals.
///
/// Only the *highest* ordinal for `kind` can be deleted this way, and doing
/// so goes through the same `apply_scaling_deltas` path the "scale down"
/// button uses (delta -1) rather than just unlinking the file. Riku's
/// scaling model is "N contiguous workers, 1..=N" — there's no such thing as
/// a sparse hole at ordinal 2 while 1 and 3 exist — so deleting a
/// non-highest ordinal directly would leave the SCALING file still saying
/// the old count, and the very next deploy/config-set would silently
/// recreate exactly the instance the operator just removed. Restart a
/// non-highest crashed ordinal instead (see `restart_worker`); to remove it
/// permanently, scale down to it first.
pub fn delete_worker(app: &str, paths: &RikuPaths, kind: &str, ordinal: u32) -> Result<()> {
    let app = validate_app_name(app)?;
    validate_kind(kind)?;

    let current = read_scaling_count(paths, &app, kind)?;
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
    apply_scaling_deltas(&app, paths, &deltas, &workers)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn paths_for(root: &std::path::Path) -> RikuPaths {
        let paths = RikuPaths::from_dirs(root.join(".riku"), root);
        fs::create_dir_all(&paths.workers_available).unwrap();
        fs::create_dir_all(&paths.workers_enabled).unwrap();
        paths
    }

    fn seed_worker(paths: &RikuPaths, app: &str, kind: &str, ordinal: u32) {
        let filename = format!("{app}-{kind}-{ordinal}.toml");
        let available = paths.workers_available.join(&filename);
        fs::write(&available, "placeholder").unwrap();
        std::os::unix::fs::symlink(&available, paths.workers_enabled.join(&filename)).unwrap();
    }

    #[test]
    fn restart_swaps_the_symlink_without_touching_content() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_for(tmp.path());
        seed_worker(&paths, "myapp", "web", 2);

        restart_worker("myapp", &paths, "web", 2).unwrap();

        let enabled = paths.workers_enabled.join("myapp-web-2.toml");
        assert!(enabled.symlink_metadata().unwrap().file_type().is_symlink());
        assert_eq!(fs::read_to_string(&enabled).unwrap(), "placeholder");
    }

    #[test]
    fn restart_errors_when_config_was_scaled_away() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_for(tmp.path());

        let err = restart_worker("myapp", &paths, "web", 5).unwrap_err();
        assert!(err.to_string().contains("scaled away"));
    }

    fn seed_scaling(paths: &RikuPaths, app: &str, kind: &str, count: u32) {
        let dir = paths.env_root.join(app);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("SCALING"), format!("{kind}={count}\n")).unwrap();
    }

    #[test]
    fn delete_removes_the_highest_ordinal_and_updates_scaling() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_for(tmp.path());
        seed_worker(&paths, "myapp", "web", 1);
        seed_worker(&paths, "myapp", "web", 2);
        seed_scaling(&paths, "myapp", "web", 2);

        delete_worker("myapp", &paths, "web", 2).unwrap();

        assert!(paths.workers_enabled.join("myapp-web-1.toml").exists());
        // apply_scaling_deltas only unlinks the enabled symlink (stopping the
        // process) -- the workers_available file is left behind, same as
        // any other scale-down; that's an existing, unrelated behavior.
        assert!(!paths.workers_enabled.join("myapp-web-2.toml").exists());
        assert_eq!(read_scaling_count(&paths, "myapp", "web").unwrap(), 1);
    }

    #[test]
    fn delete_refuses_a_non_highest_ordinal() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_for(tmp.path());
        seed_worker(&paths, "myapp", "web", 1);
        seed_worker(&paths, "myapp", "web", 2);
        seed_scaling(&paths, "myapp", "web", 2);

        let err = delete_worker("myapp", &paths, "web", 1).unwrap_err();
        assert!(err.to_string().contains("isn't the highest instance"));
        // Nothing was touched.
        assert!(paths.workers_enabled.join("myapp-web-1.toml").exists());
        assert!(paths.workers_enabled.join("myapp-web-2.toml").exists());
    }

    #[test]
    fn rejects_invalid_kind() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_for(tmp.path());
        assert!(restart_worker("myapp", &paths, "../etc", 1).is_err());
        assert!(delete_worker("myapp", &paths, "web/../x", 1).is_err());
    }
}
