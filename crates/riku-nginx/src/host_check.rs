//! Whether a router install can go ahead on the host's nginx. Deploy and
//! router apply check this before changing anything, and `riku doctor`
//! reports it.
//!
//! The include loads every `*.conf` in `nginx_root`, so a file there that is
//! no app's config is loaded too, and a bad one fails every reload.
//! Removal is not checked: a destroy that failed partway leaves such a
//! config, and removing it must still work.

use std::fs;
use std::path::PathBuf;

use anyhow::{anyhow, Result};

use crate::config::RikuPaths;

/// What [`check_host_ready`] found on a host with nothing to fix.
#[derive(Debug, PartialEq)]
pub enum HostReadiness {
    /// No stray config in `nginx_root`.
    Ready,
    /// The nginx program is not installed, so there is nothing to keep in
    /// step. Holds where it was looked for.
    NoNginx(String),
}

/// Whether `file_name` is `<app>.conf` for an app that exists.
fn is_app_config(file_name: &str, paths: &RikuPaths) -> bool {
    match file_name.strip_suffix(".conf") {
        Some(app) if !app.is_empty() && !app.contains('/') => paths.app_root.join(app).is_dir(),
        _ => false,
    }
}

/// `*.conf` files in `nginx_root` that are no existing app's config.
pub fn stray_site_configs(paths: &RikuPaths) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(&paths.nginx_root) else {
        return Vec::new();
    };
    entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            name.ends_with(".conf") && !is_app_config(&name, paths)
        })
        .map(|entry| entry.path())
        .collect()
}

/// Fails when `nginx_root` holds a `*.conf` that is no app's config, naming
/// it. Without the nginx program there is nothing to check, and the result
/// says why.
pub fn check_host_ready(paths: &RikuPaths) -> Result<HostReadiness> {
    let nginx = &paths.reload_programs.nginx;
    if !nginx.is_absolute() || !nginx.is_file() {
        return Ok(HostReadiness::NoNginx(format!(
            "nginx program not found ({})",
            nginx.display()
        )));
    }
    match stray_site_configs(paths).first() {
        Some(stray) => Err(anyhow!(
            "nginx loads {}, which is no riku app's config, remove that file",
            stray.display()
        )),
        None => Ok(HostReadiness::Ready),
    }
}

#[cfg(test)]
#[path = "host_check_tests.rs"]
mod tests;
