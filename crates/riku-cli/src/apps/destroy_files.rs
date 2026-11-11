//! The files and directories `riku destroy` deletes once the route is gone.
//! The app directory goes last: destroy looks the app up there, so until it
//! is deleted a failed destroy can be retried.

use anyhow::{anyhow, Result};
use riku_supervisor::config::app_worker_configs;
use std::fs;
use std::path::Path;

use crate::config::RikuPaths;
use crate::nginx::remove_static;
use crate::util::{display, ensure_path_within};

fn remove_dir(path: &Path) -> Result<()> {
    display::detail(&format!("Removing {}", path.display()));
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(e) => Err(anyhow!("failed to remove {}: {e}", path.display())),
    }
}

fn remove_dir_if_present(path: &Path) -> Result<()> {
    if path.exists() {
        return remove_dir(path);
    }
    Ok(())
}

fn remove_file(path: &Path) -> Result<()> {
    display::detail(&format!("Removing {}", path.display()));
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) => Err(anyhow!("failed to remove {}: {e}", path.display())),
    }
}

/// The `repos/<app>.git` symlink a push made to a bare repo kept outside
/// riku. Only the link goes, never the repo it points to.
fn remove_repo_link(paths: &RikuPaths, app: &str) -> Result<()> {
    let link = paths.git_root.join(format!("{app}.git"));
    match fs::symlink_metadata(&link) {
        Ok(meta) if meta.file_type().is_symlink() => remove_file(&link),
        _ => Ok(()),
    }
}

/// The enabled links go first: they are attributed by reading the config
/// they point to, which is gone once `workers_available` is cleared.
#[allow(clippy::question_mark)]
fn remove_worker_configs(paths: &RikuPaths, app: &str) -> Result<()> {
    for dir in [&paths.workers_enabled, &paths.workers_available] {
        for config in app_worker_configs(dir, app) {
            if let Err(e) = remove_file(&config) {
                return Err(e);
            }
        }
    }
    Ok(())
}

#[allow(clippy::question_mark)]
fn remove_acme_certs(paths: &RikuPaths, app: &str) -> Result<()> {
    let acme_link = paths.acme_cert_links.join(app);
    if !acme_link.exists() {
        return Ok(());
    }
    let acme_certs = fs::canonicalize(&acme_link).unwrap_or_else(|_| acme_link.clone());
    if acme_certs.exists() {
        if ensure_path_within(&acme_certs, &paths.riku_root).is_ok()
            || ensure_path_within(&acme_certs, &paths.acme_sh_root).is_ok()
        {
            if let Err(e) = remove_dir(&acme_certs) {
                return Err(e);
            }
        } else {
            display::warn(&format!(
                "ACME cert path '{}' points outside expected directories, skipping removal",
                acme_certs.display()
            ));
        }
    }
    display::detail(&format!("Removing {}", acme_link.display()));
    if let Err(e) = fs::remove_file(&acme_link) {
        tracing::warn!("Could not remove ACME symlink {:?}: {}", acme_link, e);
    }
    Ok(())
}

/// Delete worker configs, the repo link, the env, git and log directories,
/// the ACME certs and the published static files, then `apps/<app>` last.
#[allow(clippy::question_mark)]
pub(super) fn remove_app_files(paths: &RikuPaths, app: &str) -> Result<()> {
    if let Err(e) = remove_worker_configs(paths, app) {
        return Err(e);
    }
    if let Err(e) = remove_repo_link(paths, app) {
        return Err(e);
    }
    for dir in [&paths.env_root, &paths.git_root, &paths.log_root] {
        if let Err(e) = remove_dir_if_present(&dir.join(app)) {
            return Err(e);
        }
    }
    if let Err(e) = remove_acme_certs(paths, app) {
        return Err(e);
    }
    if let Err(e) = remove_static(app, paths) {
        return Err(e);
    }
    remove_dir_if_present(&paths.app_root.join(app))
}
