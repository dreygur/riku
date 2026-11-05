//! Root-only host setup for the nginx router.
//!
//! riku writes every site config into the deploy user's `~/.riku/nginx` and
//! never under `/etc/nginx`. Setup installs the one root-owned file that
//! loads that directory, `/etc/nginx/conf.d/riku.conf`, and puts the nginx
//! worker user in the `riku-sockets` group (see [`super::nginx_worker`]).
//!
//! Security: the include file's text is built from the riku root path only
//! and refused if that path holds nginx syntax (see
//! [`crate::nginx::include_line`]), and it is written only into a directory
//! root owns and nobody else can write. No directory under `/etc/nginx`
//! changes owner or mode.

use std::fs;
use std::io::ErrorKind;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{anyhow, Result};

use crate::config::{NginxHostPaths, RikuPaths};
use crate::nginx::{include_file_contents, needs_catch_all, write_include_file};
use crate::util::echo;

use super::deploy_user::DeployUser;
use super::nginx_worker::grant_nginx_sockets;

/// Whether the include file should carry riku's catch-all server, from what
/// `nginx -T` shows. When nginx cannot dump its config the answer is no, so
/// riku never risks a second `default_server` that would stop nginx.
fn wants_catch_all(paths: &RikuPaths) -> bool {
    let dumped = Command::new(&paths.reload_programs.nginx)
        .arg("-T")
        .stdin(Stdio::null())
        .output();
    match dumped {
        Ok(output) if output.status.success() => needs_catch_all(
            &String::from_utf8_lossy(&output.stdout),
            &paths.nginx_include_file,
        ),
        _ => {
            echo(
                "      warning: nginx -T failed, so no catch-all server was added for unknown hosts",
                "yellow",
            );
            false
        }
    }
}

/// Write the include file unless it already holds the same text. Returns
/// whether it was written.
#[allow(clippy::question_mark)]
fn install_include(paths: &RikuPaths) -> Result<bool> {
    let catch_all = wants_catch_all(paths);
    let contents = match include_file_contents(paths, catch_all) {
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
    match write_include_file(paths, catch_all) {
        Ok(()) => Ok(true),
        Err(e) => Err(e),
    }
}

fn load_riku_sites(paths: &RikuPaths) -> Result<()> {
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

/// Root's uid, the owner a directory root writes into must have.
pub(super) const ROOT_UID: u32 = 0;

/// Refuses `dir` for a root write unless it is a real directory owned by
/// `owner` that no group or other user can write. Otherwise another
/// account could swap the file root writes for a symlink between check and
/// rename. `owner` is [`ROOT_UID`] in production; tests pass their own uid.
pub(super) fn check_write_dir(dir: &Path, owner: u32) -> Result<()> {
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
/// nginx is not reloaded here. The next deploy reloads it.
#[allow(clippy::question_mark)]
pub fn setup_nginx_host(user: &DeployUser) -> Result<()> {
    // Root writes under these paths, so the environment (kept
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
    if let Err(e) = load_riku_sites(&paths) {
        return Err(e);
    }
    grant_nginx_sockets()
}

#[cfg(test)]
#[path = "nginx_host_tests.rs"]
mod tests;
