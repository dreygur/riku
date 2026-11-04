//! Root-only host setup for the nginx router.
//!
//! riku writes every site config into the deploy user's `~/.riku/nginx` and
//! never under `/etc/nginx`. Setup removes the per-app symlinks older riku
//! versions put in `/etc/nginx/sites-enabled`, which would otherwise load
//! each site a second time, and the `test_nginx.conf` they could leave in
//! `~/.riku/nginx`, then installs the one root-owned file that loads that
//! directory, `/etc/nginx/conf.d/riku.conf`.
//!
//! Security: the include file's text is built from the riku root path only
//! and refused if that path holds nginx syntax (see
//! [`crate::nginx::include_line`]), and it is written only into a directory
//! root owns and nobody else can write. Root removes only symlinks in
//! sites-enabled whose resolved target is inside the deploy user's nginx
//! directory. The stale file in the deploy user's home is removed as that
//! user, so a planted symlink cannot steer a root delete. No directory
//! under `/etc/nginx` changes owner or mode.

use std::fs;
use std::io::ErrorKind;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use anyhow::{anyhow, Result};

use crate::config::{NginxHostPaths, RikuPaths};
use crate::nginx::{
    include_file_contents, legacy_site_links, write_include_file, STALE_VALIDATION_FILE,
};
use crate::util::echo;

use super::deploy_user::DeployUser;

/// Write the include file unless it already holds the same text. Returns
/// whether it was written.
#[allow(clippy::question_mark)]
fn install_include(paths: &RikuPaths) -> Result<bool> {
    let contents = match include_file_contents(paths) {
        Ok(contents) => contents,
        Err(e) => return Err(e),
    };
    let file = &paths.nginx_include_file;
    match fs::read_to_string(file) {
        Ok(current) if current == contents => return Ok(false),
        Ok(_) => {}
        Err(e) if e.kind() == ErrorKind::NotFound => {}
        Err(e) => return Err(anyhow!("cannot read {}: {e}", file.display())),
    }
    match write_include_file(paths) {
        Ok(()) => Ok(true),
        Err(e) => Err(e),
    }
}

/// Remove the sites-enabled symlinks an older riku made. Returns how many.
fn remove_legacy_links(paths: &RikuPaths) -> Result<usize> {
    let links = legacy_site_links(paths);
    for link in &links {
        if let Err(e) = fs::remove_file(link) {
            return Err(anyhow!(
                "cannot remove old riku link {}: {e}",
                link.display()
            ));
        }
    }
    Ok(links.len())
}

/// Remove the old links first, so nginx never sees a site twice, then
/// install the include.
fn load_riku_sites(paths: &RikuPaths) -> Result<()> {
    match remove_legacy_links(paths) {
        Ok(0) => {}
        Ok(removed) => echo(
            &format!(
                "      removed {removed} old riku link(s) from {}",
                paths.nginx_sites_enabled.display()
            ),
            "green",
        ),
        Err(e) => return Err(e),
    }
    match install_include(paths) {
        Ok(true) => echo(
            &format!(
                "      {} loads riku sites",
                paths.nginx_include_file.display()
            ),
            "green",
        ),
        Ok(false) => {}
        Err(e) => return Err(e),
    }
    Ok(())
}

/// Root's uid, the owner the include directory must have.
const ROOT_UID: u32 = 0;

/// Refuses `dir` for a root write unless it is a real directory owned by
/// `owner` that no group or other user can write. Otherwise another
/// account could swap the include file for a symlink between check and
/// rename. `owner` is [`ROOT_UID`] in production; tests pass their own uid.
fn check_write_dir(dir: &Path, owner: u32) -> Result<()> {
    let meta = match dir.symlink_metadata() {
        Ok(meta) => meta,
        Err(e) => return Err(anyhow!("cannot inspect {}: {e}", dir.display())),
    };
    if !meta.is_dir() {
        return Err(anyhow!(
            "refusing to write into {}: not a directory (a symlink?)",
            dir.display()
        ));
    }
    if meta.uid() != owner {
        return Err(anyhow!(
            "refusing to write into {}: it is owned by uid {}, not root",
            dir.display(),
            meta.uid()
        ));
    }
    if meta.mode() & 0o022 != 0 {
        return Err(anyhow!(
            "refusing to write into {}: it is group or world writable (mode {:o})",
            dir.display(),
            meta.mode() & 0o777
        ));
    }
    Ok(())
}

fn missing_dir_warning(paths: &RikuPaths, dir: &Path) -> String {
    if !paths.reload_programs.nginx.is_file() {
        return "      warning: nginx is not installed; after installing it, re-run: sudo riku init"
            .to_string();
    }
    format!(
        "      warning: {} does not exist: create it, include {}/*.conf inside the http block \
         of nginx.conf, then re-run: sudo riku init",
        dir.display(),
        dir.display()
    )
}

/// Make the system nginx load riku's sites. Root only. A host without the
/// include directory gets a warning saying what to do.
///
/// nginx is not reloaded here. The next deploy reloads it, and the configs
/// it then loads are the ones the old symlinks pointed at.
#[allow(clippy::question_mark)]
pub fn setup_nginx_host(user: &DeployUser) -> Result<()> {
    // Root writes and deletes under these paths, so the environment (kept
    // by `sudo -E`) must not pick them.
    let paths = RikuPaths::with_nginx_host(
        user.home.join(".riku"),
        &user.home,
        NginxHostPaths::system(),
    );
    let file = &paths.nginx_include_file;
    let Some(dir) = file.parent() else {
        return Err(anyhow!(
            "nginx include path {} has no directory",
            file.display()
        ));
    };
    if !dir.is_dir() {
        echo(&missing_dir_warning(&paths, dir), "yellow");
        return Ok(());
    }
    if let Err(e) = check_write_dir(dir, ROOT_UID) {
        return Err(e);
    }

    let stale = paths.nginx_root.join(STALE_VALIDATION_FILE);
    if let Err(e) = user.remove_file(&stale) {
        return Err(e);
    }
    load_riku_sites(&paths)
}

#[cfg(test)]
#[path = "nginx_host_tests.rs"]
mod tests;
