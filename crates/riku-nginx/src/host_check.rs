//! Whether the host's nginx loads riku's sites exactly once. Deploy and
//! destroy check this before changing anything, and `riku doctor` reports it.
//!
//! What breaks that: the include file or its directory missing, symlinks an
//! older riku left in sites-enabled (see [`crate::host_links`]), and, for a
//! change that installs a config, a `*.conf` in `nginx_root` that is no
//! app's config (the include loads it, and a bad one fails every reload).
//! Removal skips that last check: a destroy that failed partway leaves such
//! a config, and removing it must still work.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};

use crate::config::RikuPaths;
use crate::host::includes_riku_sites;
use crate::host_links::{is_app_config, legacy_site_links};

/// The `nginx -t` wrapper older riku versions wrote into `nginx_root` and
/// left behind when interrupted. Root init deletes it.
pub const STALE_VALIDATION_FILE: &str = "test_nginx.conf";

/// The router change a check is for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RouteChange {
    /// Writing or enabling a site config (deploy, apply). Also what doctor
    /// checks.
    Install,
    /// Removing a site config (destroy).
    Remove,
}

/// What [`check_host_ready`] found on a host with nothing to fix.
#[derive(Debug, PartialEq)]
pub enum HostReadiness {
    /// nginx loads `nginx_root` through the include file.
    Ready,
    /// The nginx program is not installed, so there is nothing to keep in
    /// step. Holds where it was looked for.
    NoNginx(String),
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

fn stray_config_error(stray: &Path) -> anyhow::Error {
    let named = stray
        .file_name()
        .is_some_and(|n| n == STALE_VALIDATION_FILE);
    let fix = if named {
        "run: sudo riku init"
    } else {
        "remove that file"
    };
    anyhow!(
        "nginx loads {}, which is no riku app's config, {fix}",
        stray.display()
    )
}

fn include_dir_error(paths: &RikuPaths) -> Option<anyhow::Error> {
    let file = &paths.nginx_include_file;
    match file.parent() {
        Some(dir) if dir.is_dir() => None,
        Some(dir) => Some(anyhow!(
            "{} does not exist, so nginx cannot load riku sites: create it, include \
             {}/*.conf inside the http block of nginx.conf, then run: sudo riku init",
            dir.display(),
            dir.display()
        )),
        None => Some(anyhow!(
            "nginx include path {} has no directory",
            file.display()
        )),
    }
}

/// Fails unless nginx would load riku's sites exactly once, naming the
/// file, link or directory at fault and how to fix it. Without the nginx
/// program there is nothing to check, and the result says why.
pub fn check_host_ready(paths: &RikuPaths, change: RouteChange) -> Result<HostReadiness> {
    let nginx = &paths.reload_programs.nginx;
    if !nginx.is_absolute() || !nginx.is_file() {
        return Ok(HostReadiness::NoNginx(format!(
            "nginx program not found ({})",
            nginx.display()
        )));
    }
    if let Some(e) = include_dir_error(paths) {
        return Err(e);
    }
    let old_links = legacy_site_links(paths);
    if let Some(first) = old_links.first() {
        return Err(anyhow!(
            "{} old riku link(s) in {}, such as {}, would load sites twice, run: sudo riku init",
            old_links.len(),
            paths.nginx_sites_enabled.display(),
            first.display()
        ));
    }
    if !includes_riku_sites(paths) {
        return Err(anyhow!(
            "{} is missing, nginx does not load riku sites, run: sudo riku init",
            paths.nginx_include_file.display()
        ));
    }
    if change == RouteChange::Remove {
        return Ok(HostReadiness::Ready);
    }
    match stray_site_configs(paths).first() {
        Some(stray) => Err(stray_config_error(stray)),
        None => Ok(HostReadiness::Ready),
    }
}

#[cfg(test)]
#[path = "host_check_tests.rs"]
mod tests;
