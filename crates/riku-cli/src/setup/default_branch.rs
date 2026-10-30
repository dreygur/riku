//! The branch new app repos start on.
//!
//! Riku creates bare repos with plain `git init --bare`, so the deploy
//! user's git `init.defaultBranch` decides their HEAD. A HEAD that names a
//! branch the push never sends leaves the first clone with nothing checked
//! out. Init writes the setting to `~/.gitconfig` directly (not `--global`,
//! which could follow `XDG_CONFIG_HOME` somewhere else).
//!
//! Security: `RIKU_DEFAULT_BRANCH` is checked with
//! `git check-ref-format --branch` and passed as argv, never through a shell.

use std::path::Path;
use std::process::{Command, Output};

use anyhow::{anyhow, Result};

use crate::util::echo;

/// Init reads this to override the branch.
pub const BRANCH_ENV: &str = "RIKU_DEFAULT_BRANCH";
const DEFAULT_BRANCH: &str = "main";
const CONFIG_KEY: &str = "init.defaultBranch";
/// `git config --get` exits 1 when the key is unset.
const GIT_CONFIG_UNSET: i32 = 1;

fn git(home: &Path, args: &[&str]) -> Result<Output> {
    let config = home.join(".gitconfig");
    let output = Command::new("git")
        .args(["config", "--file"])
        .arg(&config)
        .args(args)
        .output();
    match output {
        Ok(output) => Ok(output),
        Err(e) => Err(anyhow!("failed to run git config: {e}")),
    }
}

#[allow(clippy::question_mark)]
fn read_default_branch(home: &Path) -> Result<Option<String>> {
    let output = match git(home, &["--get", CONFIG_KEY]) {
        Ok(output) => output,
        Err(e) => return Err(e),
    };
    if output.status.code() == Some(GIT_CONFIG_UNSET) {
        return Ok(None);
    }
    if !output.status.success() {
        return Err(anyhow!(
            "git config --get {CONFIG_KEY} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(Some(
        String::from_utf8_lossy(&output.stdout).trim().to_string(),
    ))
}

fn check_branch_name(branch: &str) -> Result<()> {
    let status = Command::new("git")
        .args(["check-ref-format", "--branch", branch])
        .output();
    match status {
        Ok(output) if output.status.success() => Ok(()),
        Ok(_) => Err(anyhow!("invalid branch name in {BRANCH_ENV}: {branch:?}")),
        Err(e) => Err(anyhow!("failed to run git check-ref-format: {e}")),
    }
}

#[allow(clippy::question_mark)]
fn write_default_branch(home: &Path, branch: &str) -> Result<()> {
    let output = match git(home, &[CONFIG_KEY, branch]) {
        Ok(output) => output,
        Err(e) => return Err(e),
    };
    if !output.status.success() {
        return Err(anyhow!(
            "git config {CONFIG_KEY} {branch} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(())
}

/// Set the default branch for repos created under `home`.
///
/// `requested` is the `RIKU_DEFAULT_BRANCH` value. Without one, a branch
/// already in the user's git config wins over `main`.
#[allow(clippy::question_mark)]
pub fn set_default_branch(home: &Path, requested: Option<String>) -> Result<()> {
    let branch = match requested {
        Some(branch) => branch,
        None => match read_default_branch(home) {
            Ok(Some(existing)) => {
                echo(&format!("      ✓ Default branch: {existing}"), "green");
                return Ok(());
            }
            Ok(None) => DEFAULT_BRANCH.to_string(),
            Err(e) => return Err(e),
        },
    };
    if let Err(e) = check_branch_name(&branch) {
        return Err(e);
    }
    if let Err(e) = write_default_branch(home, &branch) {
        return Err(e);
    }
    echo(&format!("      ✓ Default branch: {branch}"), "green");
    Ok(())
}

#[cfg(test)]
#[path = "default_branch_tests.rs"]
mod tests;
