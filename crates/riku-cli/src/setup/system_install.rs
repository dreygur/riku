//! Telling a plain `riku init` that root already installed riku here.
//!
//! On such a host a plain init would build a second, user level install
//! next to the system one: its own `~/.riku`, a user systemd unit running
//! a second supervisor, and a forced command in the caller's
//! `authorized_keys`. It is refused before anything is changed.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Error, Result};
use nix::unistd::{Uid, User};

use super::deploy_user::deploy_user_name;
use super::system_service::SYSTEMD_DIR;

/// What marks this host as having a system install.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum SystemInstall {
    /// The system `riku.service` exists, perhaps left over from an earlier
    /// install.
    Unit(PathBuf),
    /// The deploy account exists and is not the caller.
    DeployAccount(String),
}

/// What marks a system install on this host, or `None` when nothing does.
/// `unit` is the system `riku.service`; `deploy_uid` is the uid of the
/// deploy account, `None` when no such account exists.
pub(super) fn system_install(
    unit: &Path,
    deploy_uid: Option<Uid>,
    me: Uid,
) -> Option<SystemInstall> {
    if unit.exists() {
        return Some(SystemInstall::Unit(unit.to_path_buf()));
    }
    match deploy_uid {
        Some(uid) if uid != me => Some(SystemInstall::DeployAccount(deploy_user_name())),
        _ => None,
    }
}

/// The refusal, naming the blocker found and the two ways out: update the
/// system install, or clear that blocker for a user only install.
pub(super) fn refusal(found: &SystemInstall) -> Error {
    let (reason, user_only) = match found {
        SystemInstall::Unit(unit) => (
            format!("{} exists", unit.display()),
            format!(
                "if that unit is left over and you want a user only install, remove {}",
                unit.display()
            ),
        ),
        SystemInstall::DeployAccount(name) => (
            format!("the deploy account {name} exists"),
            "for a user only install, set RIKU_USER to another deploy user name".to_string(),
        ),
    };
    anyhow!(
        "riku is already installed for the whole system here ({reason}). \
         A plain riku init would build a second install for this account. \
         Run sudo riku init to update the system install, or, {user_only}."
    )
}

/// Fail when root already installed riku on this host. Called by a plain
/// (not root) init before it changes anything.
pub(super) fn refuse_beside_system_install() -> Result<()> {
    let unit = Path::new(SYSTEMD_DIR).join("riku.service");
    let deploy_uid = match User::from_name(&deploy_user_name()) {
        Ok(Some(user)) => Some(user.uid),
        _ => None,
    };
    match system_install(&unit, deploy_uid, Uid::current()) {
        Some(found) => Err(refusal(&found)),
        None => Ok(()),
    }
}

#[cfg(test)]
#[path = "system_install_tests.rs"]
mod tests;
