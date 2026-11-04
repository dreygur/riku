//! Removing an app's files from `nginx_root`. Nothing outside it is touched.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use riku_util::file_names::copy_socket;

use super::install::{site_file, staged_file};
use crate::config::RikuPaths;

/// What [`remove_nginx_files`] did.
#[derive(Debug)]
pub struct NginxRemoval {
    /// A live config was removed, so nginx needs a reload.
    pub was_live: bool,
    /// Every socket, cert, key or staged config that could not be removed,
    /// in one error. The live config is gone either way when `was_live` is
    /// set.
    pub leftover_error: Option<anyhow::Error>,
}

/// `symlink_metadata`, not `exists`: a dangling `<app>.conf` link is still
/// an entry nginx's glob matches, and `exists` follows the link.
fn is_present(file: &Path) -> bool {
    file.symlink_metadata().is_ok()
}

fn remove_if_present(file: &Path) -> Result<()> {
    if !is_present(file) {
        return Ok(());
    }
    match fs::remove_file(file) {
        Ok(()) => Ok(()),
        Err(e) => Err(anyhow!("failed to remove {file:?}: {e}")),
    }
}

/// The `<app>@<n>.sock` sockets of `app`'s uWSGI copies in `dir`.
fn copy_sockets(app: &str, dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .and_then(copy_socket::parse)
                .is_some_and(|(owner, _)| owner == app)
        })
        .collect()
}

/// Remove an app's live config first, then its staged config, sockets, cert
/// and key. Fails only when the live config cannot be removed, since then
/// nothing nginx loads has changed. A later failure is returned in
/// [`NginxRemoval::leftover_error`] so the caller still reloads.
///
/// The caller does the reload: the deploy user may not signal the nginx
/// master, so it goes through the router reload.
#[allow(clippy::question_mark)]
pub fn remove_nginx_files(app: &str, paths: &RikuPaths) -> Result<NginxRemoval> {
    let config_file = site_file(app, paths);
    let was_live = is_present(&config_file);
    if let Err(e) = remove_if_present(&config_file) {
        return Err(e);
    }

    let side_files = [staged_file(app, paths)]
        .into_iter()
        .chain(["sock", "key", "crt"].map(|ext| paths.nginx_root.join(format!("{app}.{ext}"))))
        .chain(copy_sockets(app, &paths.nginx_root));
    let failures: Vec<String> = side_files
        .filter_map(|file| remove_if_present(&file).err())
        .map(|e| e.to_string())
        .collect();
    let leftover_error = if failures.is_empty() {
        None
    } else {
        Some(anyhow!("{}", failures.join("; ")))
    };

    Ok(NginxRemoval {
        was_live,
        leftover_error,
    })
}

#[cfg(test)]
#[path = "remove_tests.rs"]
mod tests;
