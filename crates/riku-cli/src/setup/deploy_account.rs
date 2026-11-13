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
/// What older inits gave the account.
const OLD_LOGIN_SHELL: &str = "/bin/bash";

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

/// What init does about a deploy account whose login shell is not
/// [`LOGIN_SHELL`].
#[derive(Debug, PartialEq, Eq)]
pub(super) enum ShellAction {
    Run(accounts::Cmd),
    /// Change nothing; print this.
    Explain(String),
}

/// The action for deploy account `name` whose shell is `current` on `host`.
/// `None` unless `current` is [`OLD_LOGIN_SHELL`], the shell older inits
/// gave the account: any other shell was chosen by someone, not by riku.
pub(super) fn shell_action(current: &str, host: accounts::Host, name: &str) -> Option<ShellAction> {
    use accounts::{Host, Tool};
    if current != OLD_LOGIN_SHELL {
        return None;
    }
    let action = match host {
        Host::Tools(Tool::Shadow) => ShellAction::Run(accounts::Cmd::new(
            "usermod",
            &["--shell", LOGIN_SHELL, "--", name],
        )),
        Host::Tools(Tool::BusyBox) => ShellAction::Explain(format!(
            "{name}'s login shell is {current}; BusyBox has no usermod, so run: \
             apk add shadow && usermod --shell {LOGIN_SHELL} {name}"
        )),
        Host::NixOs => ShellAction::Explain(format!(
            "{name}'s login shell is {current}; riku changes no accounts on NixOS. \
             Set users.users.{name}.shell in configuration.nix if you want {LOGIN_SHELL}"
        )),
        Host::NoTools => ShellAction::Explain(format!(
            "{name}'s login shell is {current}; no usermod found to set it to {LOGIN_SHELL}"
        )),
    };
    Some(action)
}

/// Move deploy account `name` to [`LOGIN_SHELL`] when an older init gave it
/// another shell. Never fails init: a refused or failed change is printed.
pub fn ensure_login_shell(name: &str) {
    let current = match User::from_name(name) {
        Ok(Some(user)) => user.shell.to_string_lossy().into_owned(),
        Ok(None) => return,
        Err(e) => {
            display::warn(&format!("cannot read {name}'s login shell: {e}"));
            return;
        }
    };
    match shell_action(&current, accounts::this_host(), name) {
        None => {}
        Some(ShellAction::Run(cmd)) => match accounts::run(&cmd) {
            Ok(()) => display::step_done("Deploy user shell", LOGIN_SHELL),
            Err(e) => display::warn(&format!("{e:#}. Run it yourself: {cmd}")),
        },
        Some(ShellAction::Explain(text)) => display::warn(&text),
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
