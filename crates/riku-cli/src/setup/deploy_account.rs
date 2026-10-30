//! Creates the deploy account when a root `riku init` finds it missing.
//!
//! Security: the name comes from `RIKU_USER` in root's environment and goes
//! to `useradd` as argv, so it is checked against the portable login name
//! shape first. A leading `-` would otherwise read as an option.

use std::process::Command;

use anyhow::{anyhow, Result};
use nix::unistd::User;

use crate::util::echo;

const MAX_NAME_LEN: usize = 32;
/// An unmatchable password hash. `useradd` leaves `!` (locked), and OpenSSH
/// refuses every auth method for a locked account, pubkey included.
const NO_PASSWORD: &str = "*";
const LOGIN_SHELL: &str = "/bin/bash";

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

fn create_account(name: &AccountName) -> Result<()> {
    let output = Command::new("useradd")
        .args(["--create-home", "--user-group", "--shell", LOGIN_SHELL])
        .args(["--password", NO_PASSWORD, "--", name.as_str()])
        .output();
    let output = match output {
        Ok(output) => output,
        Err(e) => return Err(anyhow!("failed to run useradd: {e}")),
    };
    if !output.status.success() {
        return Err(anyhow!(
            "useradd {} failed: {}",
            name.as_str(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(())
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
    if let Err(e) = create_account(&name) {
        return Err(e);
    }
    echo(&format!("Created deploy user {}", name.as_str()), "green");
    Ok(())
}

#[cfg(test)]
#[path = "deploy_account_tests.rs"]
mod tests;
