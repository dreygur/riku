//! Root-only host setup for the nginx router.
//!
//! The router helper writes every site config into `/etc/riku/nginx` and
//! never under `/etc/nginx`. Setup creates that root owned directory, installs
//! the one root-owned file that loads it, `/etc/nginx/conf.d/riku.conf`
//! and puts the nginx worker user in the `riku-sockets` group (see
//! [`super::nginx_worker`]).
//!
//! Security: the include file's text is a constant (see
//! [`crate::nginx::include_line`]), and it is written only into a directory
//! root owns and nobody else can write. `/etc/riku` must be root owned and
//! writable by no one else before `/etc/riku/nginx` is created in it. No
//! directory under `/etc/nginx` changes owner or mode.

use std::fs;
use std::io::ErrorKind;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{anyhow, Result};

use crate::config::{NginxHostPaths, RikuPaths, CERT_ROOT, NGINX_SITE_ROOT};
use crate::nginx::{include_file_contents, needs_catch_all, write_include_file};
use crate::util::echo;

use super::deploy_user::DeployUser;
use super::nginx_worker::grant_nginx_sockets;
use super::router_dirs::{secure_cert_dirs, secure_dir};

/// A regular file owned by `root_uid` that no one else can write.
fn is_trusted_file(meta: &fs::Metadata, root_uid: u32) -> bool {
    meta.is_file() && meta.uid() == root_uid && meta.mode() & 0o022 == 0
}

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
                "nginx -T failed, so no catch-all server was added for unknown hosts",
                "yellow",
            );
            false
        }
    }
}

/// Write the include file unless it already holds the same text and is a
/// root owned regular file nobody else can write. Returns whether it was
/// written.
fn install_include(paths: &RikuPaths, root_uid: u32) -> Result<bool> {
    let catch_all = wants_catch_all(paths);
    let contents = include_file_contents(catch_all);
    let file = &paths.nginx_include_file;
    let trusted = match fs::symlink_metadata(file) {
        Ok(meta) => is_trusted_file(&meta, root_uid),
        Err(_) => false,
    };
    match fs::read_to_string(file) {
        Ok(current) if trusted && current == contents => return Ok(false),
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
    match install_include(paths, ROOT_UID) {
        Ok(true) => crate::util::display::step_done(
            "Nginx include",
            &format!("{} loads riku sites", paths.nginx_include_file.display()),
        ),
        Ok(false) => {}
        Err(e) => return Err(e),
    }
    Ok(())
}

pub(super) use riku_util::write_dir::{check_write_dir, ROOT_UID};

fn missing_dir_warning(paths: &RikuPaths, dir: &Path) -> String {
    if !paths.reload_programs.nginx.is_file() {
        return "nginx is not installed; after installing it, re-run: sudo riku init".to_string();
    }
    format!(
        "{} does not exist: create it, include {}/*.conf inside the http block \
         of nginx.conf, then re-run: sudo riku init",
        dir.display(),
        dir.display()
    )
}

/// The mode of `/etc/riku/nginx`: root writes, nginx workers read.
const SITE_ROOT_MODE: u32 = 0o755;
/// The mode of `/etc/riku/certs` and each app directory in it: root only.
const CERT_DIR_MODE: u32 = 0o700;

/// Create `site_root` (`/etc/riku/nginx`) inside a parent that only `owner`
/// can write, and give it owner `owner` and mode `0755` whether or not it
/// existed.
fn prepare_site_root(site_root: &Path, owner: u32) -> Result<()> {
    let Some(parent) = site_root.parent() else {
        return Err(anyhow!("{} has no parent directory", site_root.display()));
    };
    if let Err(e) = fs::create_dir_all(parent) {
        return Err(anyhow!("failed to create {}: {e}", parent.display()));
    }
    if let Err(e) = check_write_dir(parent, owner) {
        return Err(anyhow!("{e}"));
    }
    match fs::create_dir(site_root) {
        Ok(()) => {}
        Err(e) if e.kind() == ErrorKind::AlreadyExists => {}
        Err(e) => return Err(anyhow!("failed to create {}: {e}", site_root.display())),
    }
    secure_dir(site_root, owner, None, SITE_ROOT_MODE)
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
    if let Err(e) = prepare_site_root(Path::new(NGINX_SITE_ROOT), ROOT_UID) {
        return Err(e);
    }
    if let Err(e) = secure_cert_dirs(Path::new(CERT_ROOT), ROOT_UID, None, CERT_DIR_MODE) {
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
