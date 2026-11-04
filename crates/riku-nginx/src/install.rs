//! Putting a site config live in `nginx_root`.
//!
//! nginx loads every `nginx_root/*.conf` through the host include, so a file
//! there is live at the next reload. A config is written as
//! `<name>.conf.new`, checked with `nginx -t`, and only renamed to
//! `<name>.conf` once nginx accepts it. A rejected config is deleted and the
//! previous live one stays as it was.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};

use super::validate::validate_nginx_config;
use crate::config::RikuPaths;

/// The live config path for `name`.
pub(super) fn site_file(name: &str, paths: &RikuPaths) -> PathBuf {
    paths.nginx_root.join(format!("{name}.conf"))
}

/// The staging path for `name`, outside the `*.conf` include.
pub(super) fn staged_file(name: &str, paths: &RikuPaths) -> PathBuf {
    paths.nginx_root.join(format!("{name}.conf.new"))
}

/// Validate `contents` and make it `name`'s live config.
pub(super) fn install_site(name: &str, contents: &str, paths: &RikuPaths) -> Result<()> {
    install_checked(&site_file(name, paths), contents, paths)
}

/// Run `nginx -t` on `contents` as `name`'s config without making it live.
pub(super) fn check_site(name: &str, contents: &str, paths: &RikuPaths) -> Result<()> {
    let target = site_file(name, paths);
    let staged = staged_file(name, paths);
    if let Err(e) = crate::util::write_atomic(&staged, contents.as_bytes()) {
        return Err(anyhow!("cannot write {staged:?}: {e}"));
    }
    let checked = validate_nginx_config(&staged, &paths.reload_programs.nginx);
    let _ = fs::remove_file(&staged);
    match checked {
        Ok(()) => Ok(()),
        Err(e) => Err(anyhow!("nginx rejected {}: {e}", target.display())),
    }
}

/// Validate `contents` and write it to `target` through a staged copy
/// `<target>.new`. `target` may be a name the include does not load.
pub(super) fn install_checked(target: &Path, contents: &str, paths: &RikuPaths) -> Result<()> {
    let mut staged_name = target.file_name().unwrap_or_default().to_os_string();
    staged_name.push(".new");
    let staged = target.with_file_name(staged_name);
    if let Err(e) = crate::util::write_atomic(&staged, contents.as_bytes()) {
        return Err(anyhow!("cannot write {staged:?}: {e}"));
    }
    if let Err(e) = validate_nginx_config(&staged, &paths.reload_programs.nginx) {
        let _ = fs::remove_file(&staged);
        return Err(anyhow!("nginx rejected {}: {e}", target.display()));
    }
    if let Err(e) = fs::rename(&staged, target) {
        let _ = fs::remove_file(&staged);
        return Err(anyhow!("cannot move {staged:?} to {target:?}: {e}"));
    }
    Ok(())
}

#[cfg(test)]
#[path = "install_tests.rs"]
mod tests;
