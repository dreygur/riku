//! Installs the one riku binary, `/usr/local/bin/riku`, for every user.
//!
//! Security: root writes only [`ROOT_BINARY`], and only into a directory
//! that is root owned and closed to group and other writers (see
//! `check_write_dir`), so no other account can plant a symlink in the way.
//! The source is `/proc/self/exe`, the image root is actually running, not
//! a path another account could swap after launch, and root refuses to
//! install a running binary that the deploy user owns. The installed file
//! is root owned and closed to writers, so the deploy user cannot change
//! the binary that units, hooks and forced commands start.
//!
//! That is the extent of it. The hooks and `authorized_keys` live in the
//! deploy user's tree and stay writable by the deploy user, so the deploy
//! user can point them at any program. It runs with the deploy user's own
//! privilege, never root's.

use anyhow::{anyhow, Result};
use nix::unistd::User;
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;

use crate::util::{display, echo, ROOT_BINARY};

use super::deploy_user::deploy_user_name;
use super::nginx_host::{check_write_dir, ROOT_UID};

/// The new binary's name beside the target until it is renamed into place.
const STAGED_NAME: &str = ".riku.new";
const BINARY_MODE: u32 = 0o755;
const RUNNING_EXE: &str = "/proc/self/exe";
const EXEC_BITS: u32 = 0o111;
const WRITE_BITS: u32 = 0o022;

/// Swap `target` for a copy of `source` by rename, never writing into
/// `target`: a running supervisor holds it open, and Linux refuses writes
/// to a busy executable (ETXTBSY). The old inode lives on until it exits.
fn replace_binary(source: &Path, target: &Path) -> Result<()> {
    let staged = target.with_file_name(STAGED_NAME);
    if let Err(e) = fs::copy(source, &staged) {
        return Err(anyhow!("failed to copy riku to {}: {e}", staged.display()));
    }
    if let Err(e) = fs::set_permissions(&staged, fs::Permissions::from_mode(BINARY_MODE)) {
        return Err(anyhow!("failed to chmod {}: {e}", staged.display()));
    }
    if let Err(e) = fs::rename(&staged, target) {
        return Err(anyhow!(
            "failed to move riku into {}: {e}",
            target.display()
        ));
    }
    Ok(())
}

/// Refuse `binary` unless it is a regular file owned by `owner`, closed to
/// group and other writers.
pub(crate) fn check_installed(binary: &Path, owner: u32) -> Result<()> {
    let meta = match binary.symlink_metadata() {
        Ok(meta) => meta,
        Err(e) => return Err(anyhow!("cannot inspect {}: {e}", binary.display())),
    };
    if !meta.is_file() {
        return Err(anyhow!(
            "{} is not a regular file, refusing to trust it",
            binary.display()
        ));
    }
    if meta.uid() != owner {
        return Err(anyhow!(
            "{} is owned by uid {}, not root: the deploy user could replace it. \
             Remove it and run `sudo riku init` again",
            binary.display(),
            meta.uid()
        ));
    }
    if meta.mode() & WRITE_BITS != 0 {
        return Err(anyhow!(
            "{} is group or world writable (mode {:o}). Fix it with chmod 755, \
             then run `sudo riku init` again",
            binary.display(),
            meta.mode() & 0o777
        ));
    }
    Ok(())
}

/// Refuse a `source` the deploy user owns: it could have been written by an
/// app. `deploy_uid` is `None` when the account does not exist yet.
fn check_source_owner(source: &Path, deploy_uid: Option<u32>) -> Result<()> {
    let meta = match fs::metadata(source) {
        Ok(meta) => meta,
        Err(e) => return Err(anyhow!("cannot inspect {}: {e}", source.display())),
    };
    if Some(meta.uid()) == deploy_uid {
        return Err(anyhow!(
            "the running riku binary is owned by the deploy user, so an app could have \
             written it. Run `sudo riku init` from a copy that root owns"
        ));
    }
    Ok(())
}

/// Copy `source` over `target` after checking that `target`'s directory is
/// owned by `owner` and closed to other writers. When `source` already is
/// `target`, nothing is copied but the file must pass [`check_installed`].
#[allow(clippy::question_mark)]
fn install_at(source: &Path, target: &Path, owner: u32) -> Result<()> {
    if let (Ok(from), Ok(to)) = (fs::canonicalize(source), fs::canonicalize(target)) {
        if from == to {
            return check_installed(target, owner);
        }
    }
    let Some(dir) = target.parent() else {
        return Err(anyhow!("{} has no parent directory", target.display()));
    };
    if let Err(e) = check_write_dir(dir, owner) {
        return Err(e);
    }
    let replacing = target.exists();
    if let Err(e) = replace_binary(source, target) {
        return Err(e);
    }
    display::step_done("Binary", &target.display().to_string());
    if replacing {
        echo(
            "A running supervisor still uses the old binary, restart it: sudo systemctl restart riku",
            "yellow",
        );
    }
    Ok(())
}

fn deploy_uid() -> Option<u32> {
    match User::from_name(&deploy_user_name()) {
        Ok(Some(user)) => Some(user.uid.as_raw()),
        _ => None,
    }
}

/// Install the running binary as [`ROOT_BINARY`]. Root only.
pub fn install_root_binary() -> Result<()> {
    let running = Path::new(RUNNING_EXE);
    if let Err(e) = check_source_owner(running, deploy_uid()) {
        return Err(anyhow!("refusing to install riku: {e}"));
    }
    install_at(running, Path::new(ROOT_BINARY), ROOT_UID)
}

/// Fail unless root already installed an executable [`ROOT_BINARY`]. A plain
/// `riku init` has no right to install it and says who does.
pub fn require_root_binary() -> Result<()> {
    let installed = match fs::metadata(ROOT_BINARY) {
        Ok(meta) => meta.is_file() && meta.mode() & EXEC_BITS != 0,
        Err(_) => false,
    };
    if installed {
        return Ok(());
    }
    Err(anyhow!(
        "{ROOT_BINARY} is not installed or not executable. Run `sudo riku init` once to install it."
    ))
}

#[cfg(test)]
#[path = "binary_tests.rs"]
mod tests;
