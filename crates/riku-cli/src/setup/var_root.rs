//! Root init's part of the directories nginx serves files from:
//! `/var/lib/riku`, root-owned `0755`, and in it the ACME webroot and the
//! static root, each owned by the deploy user, group `riku-sockets`, mode
//! `2750`. The deploy user writes challenge files and published static
//! copies there; the nginx worker reads them through the group and needs
//! nothing in the deploy home.
//!
//! Security: root refuses a parent of `/var/lib/riku` that is not root-owned
//! or that a group or other user can write, and refuses `/var/lib/riku`
//! itself on the same terms, so no other account can plant a symlink that
//! redirects root's chown or chmod. The two inner directories are refused
//! when they are symlinks (see [`socket_dir::prepare`]).

use std::fs;
use std::io::ErrorKind;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use anyhow::{anyhow, Result};

use crate::config::{ACME_WEBROOT, SOCKET_GROUP, STATIC_ROOT, VAR_ROOT};
use crate::util::echo;
use crate::util::socket_dir;

use super::deploy_user::DeployUser;
use super::nginx_host::{check_write_dir, ROOT_UID};

const VAR_ROOT_MODE: u32 = 0o755;

/// Make `var_root` a `0755` directory owned by `root_owner` under a parent
/// only `root_owner` can write, then prepare each of `dirs` in it for
/// `owner` and the socket group.
#[allow(clippy::question_mark)]
fn prepare_var_root(
    var_root: &Path,
    root_owner: u32,
    owner: Option<u32>,
    dirs: &[&Path],
) -> Result<()> {
    let Some(parent) = var_root.parent() else {
        return Err(anyhow!("{} has no parent directory", var_root.display()));
    };
    if let Err(e) = check_write_dir(parent, root_owner) {
        return Err(e);
    }
    match fs::create_dir(var_root) {
        Ok(()) => {}
        Err(e) if e.kind() == ErrorKind::AlreadyExists => {}
        Err(e) => return Err(anyhow!("cannot create {}: {e}", var_root.display())),
    }
    if let Err(e) = check_write_dir(var_root, root_owner) {
        return Err(e);
    }
    if let Err(e) = fs::set_permissions(var_root, fs::Permissions::from_mode(VAR_ROOT_MODE)) {
        return Err(anyhow!("cannot chmod {}: {e}", var_root.display()));
    }
    for dir in dirs {
        if let Err(e) = socket_dir::prepare(dir, owner, SOCKET_GROUP) {
            return Err(e);
        }
    }
    Ok(())
}

/// Create `/var/lib/riku` with the ACME webroot and the static root in it.
/// Root only, and run after the socket group exists. Beyond
/// [`prepare_var_root`] it is covered only by running init on a host.
#[allow(clippy::question_mark)]
pub(super) fn setup_var_root(user: &DeployUser) -> Result<()> {
    let dirs = [Path::new(ACME_WEBROOT), Path::new(STATIC_ROOT)];
    if let Err(e) = prepare_var_root(Path::new(VAR_ROOT), ROOT_UID, Some(user.uid()), &dirs) {
        return Err(e);
    }
    echo(
        &format!("      ✓ {ACME_WEBROOT} and {STATIC_ROOT} ready ({SOCKET_GROUP} reads them)"),
        "green",
    );
    Ok(())
}

#[cfg(test)]
#[path = "var_root_tests.rs"]
mod tests;
