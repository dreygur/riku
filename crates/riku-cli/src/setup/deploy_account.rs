//! Creates the deploy account when a root `riku init` finds it missing.
//!
//! Security: the name comes from `RIKU_USER` in root's environment and goes
//! to `useradd` or `adduser` as argv (see [`super::accounts`]), so it is
//! checked against the portable login name shape first. A leading `-` would
//! otherwise read as an option.

use anyhow::{anyhow, Result};
use nix::unistd::User;

use crate::util::display;

use super::accounts;

const MAX_NAME_LEN: usize = 32;
/// sshd runs the forced command through the login shell, so it must exist
/// on every host: Alpine has no bash unless someone installs it.
const LOGIN_SHELL: &str = "/bin/sh";

/// A login name `useradd` accepts on every distro: lowercase letter or `_`
/// first, then lowercase letters, digits, `_` or `-`, at most 32 bytes.
pub struct AccountName(String);

impl AccountName {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for AccountName {
    type Error = anyhow::Error;

    fn try_from(name: String) -> Result<Self> {
        let mut chars = name.chars();
        let first_ok = matches!(chars.next(), Some('a'..='z' | '_'));
        let rest_ok = chars.all(|c| matches!(c, 'a'..='z' | '0'..='9' | '_' | '-'));
        if !first_ok || !rest_ok || name.len() > MAX_NAME_LEN {
            return Err(anyhow!("invalid deploy user name {name:?}"));
        }
        Ok(Self(name))
    }
}

/// Create the deploy account `name` unless it already exists.
#[allow(clippy::question_mark)]
pub fn ensure_account(name: &str) -> Result<()> {
    match User::from_name(name) {
        Ok(Some(_)) => return Ok(()),
        Ok(None) => {}
        Err(e) => return Err(anyhow!("failed to look up user {name}: {e}")),
    }
    let name = match AccountName::try_from(name.to_string()) {
        Ok(name) => name,
        Err(e) => return Err(e),
    };
    if let Err(e) = accounts::create_user(name.as_str(), LOGIN_SHELL) {
        return Err(e);
    }
    display::step_done("Deploy user", &format!("{} created", name.as_str()));
    Ok(())
}

#[cfg(test)]
#[path = "deploy_account_tests.rs"]
mod tests;
