//! Removing the files an app leaves in the deploy user's tree: its
//! certificate files in `nginx_root` and its sockets in `socket_root`.
//! Nothing outside those two is touched. The site config itself belongs to
//! the router helper.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use riku_util::file_names::copy_socket;

use crate::config::RikuPaths;

/// Suffixes of the certificate files acme.sh and the self signed fallback
/// leave next to each other in `nginx_root`.
const CERTIFICATE_SUFFIXES: [&str; 4] = ["key", "crt", "fullchain.crt", "renew-hook"];

/// `symlink_metadata`, not `exists`: a dangling link is still an entry.
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

/// Remove `app`'s certificate files, default socket and copy sockets. Every
/// file is tried; the failures come back in one error.
pub fn remove_deploy_files(app: &str, paths: &RikuPaths) -> Result<()> {
    let certificates = CERTIFICATE_SUFFIXES
        .map(|suffix| paths.nginx_root.join(format!("{app}.{suffix}")))
        .into_iter();
    let files = certificates
        .chain([paths.socket_root.join(format!("{app}.sock"))])
        .chain(copy_sockets(app, &paths.socket_root));
    let failures: Vec<String> = files
        .filter_map(|file| remove_if_present(&file).err())
        .map(|e| e.to_string())
        .collect();
    if failures.is_empty() {
        return Ok(());
    }
    Err(anyhow!("{}", failures.join("; ")))
}

#[cfg(test)]
#[path = "remove_tests.rs"]
mod tests;
