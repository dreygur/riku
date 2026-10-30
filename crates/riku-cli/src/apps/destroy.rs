use anyhow::{anyhow, Result};
use std::fs;
use std::path::Path;

use crate::config::RikuPaths;
use crate::util::{echo, ensure_path_within, exit_if_invalid};

fn remove_dir(path: &Path) -> Result<()> {
    echo(
        &format!("--> Removing folder '{}'", path.display()),
        "yellow",
    );
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(e) => Err(anyhow!("failed to remove {}: {e}", path.display())),
    }
}

fn remove_file(path: &Path) -> Result<()> {
    echo(&format!("--> Removing file '{}'", path.display()), "yellow");
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) => Err(anyhow!("failed to remove {}: {e}", path.display())),
    }
}

/// The runtime plugin removes what it started outside the app directory.
/// A failure is reported and destroy goes on: the app is going away either
/// way, and its files are what the plugin would need to retry.
fn plugin_cleanup(paths: &RikuPaths, app: &str) {
    if let Err(e) = crate::deploy::teardown::plugin_destroy(app, paths) {
        echo(
            &format!("--> Runtime cleanup failed, containers may remain: {e}"),
            "yellow",
        );
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

#[allow(clippy::question_mark)]
fn remove_worker_configs(paths: &RikuPaths, app: &str) -> Result<()> {
    for dir in [&paths.workers_available, &paths.workers_enabled] {
        for ext in &["ini", "toml"] {
            let pattern = dir.join(format!("{}-*.{}", app, ext));
            let Ok(entries) = glob::glob(pattern.to_str().unwrap_or("")) else {
                continue;
            };
            for entry in entries.flatten() {
                if let Err(e) = remove_file(&entry) {
                    return Err(e);
                }
            }
        }
    }
    Ok(())
}

#[allow(clippy::question_mark)]
fn remove_acme_certs(paths: &RikuPaths, app: &str) -> Result<()> {
    let acme_link = paths.acme_www.join(app);
    if !acme_link.exists() {
        return Ok(());
    }
    let acme_certs = fs::canonicalize(&acme_link).unwrap_or_else(|_| acme_link.clone());
    if acme_certs.exists() {
        if ensure_path_within(&acme_certs, &paths.riku_root).is_ok()
            || ensure_path_within(&acme_certs, &paths.acme_root).is_ok()
        {
            if let Err(e) = remove_dir(&acme_certs) {
                return Err(e);
            }
        } else {
            echo(
                &format!(
                    "WARNING: ACME cert path '{}' points outside expected directories, skipping removal",
                    acme_certs.display()
                ),
                "yellow",
            );
        }
    }
    echo(
        &format!("--> Removing file '{}'", acme_link.display()),
        "yellow",
    );
    if let Err(e) = fs::remove_file(&acme_link) {
        tracing::warn!("Could not remove ACME symlink {:?}: {}", acme_link, e);
    }
    Ok(())
}

/// Destroy an app: remove directories and config files, preserve data/cache.
#[allow(clippy::question_mark)]
pub fn cmd_destroy(paths: &RikuPaths, app: &str) -> Result<()> {
    let app = match exit_if_invalid(app, &paths.app_root) {
        Ok(app) => app,
        Err(e) => return Err(e),
    };

    plugin_cleanup(paths, &app);

    for dir in [
        &paths.app_root,
        &paths.git_root,
        &paths.env_root,
        &paths.log_root,
    ] {
        let path = dir.join(&app);
        if path.exists() {
            if let Err(e) = remove_dir(&path) {
                return Err(e);
            }
        }
    }
    if let Err(e) = remove_repo_link(paths, &app) {
        return Err(e);
    }
    if let Err(e) = remove_worker_configs(paths, &app) {
        return Err(e);
    }

    echo(
        &format!("--> Removing router config for '{}'", app),
        "yellow",
    );
    if let Err(e) = crate::deploy::router::remove(&app, paths) {
        return Err(e);
    }
    if let Err(e) = remove_acme_certs(paths, &app) {
        return Err(e);
    }

    for dir in [&paths.data_root, &paths.cache_root] {
        let path = dir.join(&app);
        if path.exists() {
            echo(
                &format!("==> Preserving folder '{}'", path.display()),
                "red",
            );
        }
    }

    Ok(())
}

#[cfg(test)]
#[path = "destroy_tests.rs"]
mod tests;
