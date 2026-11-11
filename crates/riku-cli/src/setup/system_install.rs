//! Telling a plain `riku init` that root already installed riku here.
//!
//! On such a host a plain init would build a second, user level install
//! next to the system one: its own `~/.riku`, a user systemd unit running
//! a second supervisor, and a forced command in the caller's
//! `authorized_keys`. It is refused before anything is changed.

use std::path::Path;

use anyhow::{anyhow, Error, Result};
use nix::unistd::{Uid, User};

use super::deploy_user::deploy_user_name;
use super::system_service::SYSTEMD_DIR;

/// Why this host has a system install, or `None` when it has none. `unit`
/// is the system `riku.service`; `deploy_uid` is the uid of the deploy
/// account, `None` when no such account exists.
pub(super) fn system_install(unit: &Path, deploy_uid: Option<Uid>, me: Uid) -> Option<String> {
    if unit.exists() {
        return Some(format!("{} exists", unit.display()));
    }
    match deploy_uid {
        Some(uid) if uid != me => Some(format!("the deploy account {} exists", deploy_user_name())),
        _ => None,
    }
}

pub(super) fn refusal(reason: &str) -> Error {
    anyhow!(
        "riku is already installed for the whole system here ({reason}). \
         A plain riku init would build a second install for this account. \
         Run sudo riku init to update the system install."
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
        Some(reason) => Err(refusal(&reason)),
        None => Ok(()),
    }
}

#[cfg(test)]
#[path = "system_install_tests.rs"]
mod tests;
