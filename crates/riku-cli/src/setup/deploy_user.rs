//! The account apps deploy as, seen from a root `riku init`.
//!
//! Root init runs with `HOME=/root`, but git push deploys run as this user
//! and read `~/.riku` from its home. Anything deploys must see (the
//! `ROUTER` file, the Caddy site directory) goes into this user's tree and
//! is chowned to it, so the deploy user can still write there.
//!
//! Security: the user name comes from `RIKU_USER` and is passed to
//! `getent`, `id` and `chown` as a single argv entry, never through a shell.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, Result};

use crate::config::RikuPaths;

const DEFAULT_USER: &str = "deploy";

/// The deploy account: login name, home directory and primary group.
pub struct DeployUser {
    pub name: String,
    pub home: PathBuf,
    pub group: String,
}

/// The `RIKU_USER` login name, `deploy` when unset.
pub fn deploy_user_name() -> String {
    std::env::var("RIKU_USER").unwrap_or_else(|_| DEFAULT_USER.to_string())
}

/// `user`'s home directory from the passwd database.
pub fn passwd_home(user: &str) -> Option<String> {
    let output = match Command::new("getent").args(["passwd", user]).output() {
        Ok(output) if output.status.success() => output,
        _ => return None,
    };
    String::from_utf8_lossy(&output.stdout)
        .trim_end()
        .split(':')
        .nth(5)
        .map(str::to_string)
}

fn primary_group(user: &str) -> Result<String> {
    let output = match Command::new("id").args(["-gn", user]).output() {
        Ok(output) => output,
        Err(e) => return Err(anyhow!("failed to run id -gn {user}: {e}")),
    };
    if !output.status.success() {
        return Err(anyhow!(
            "id -gn {user} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

impl DeployUser {
    /// Look up the `RIKU_USER` account. Unlike the systemd unit writer, this
    /// does not guess a home: chowning into a guessed path is worse than
    /// failing.
    #[allow(clippy::question_mark)]
    pub fn lookup() -> Result<Self> {
        let name = deploy_user_name();
        let Some(home) = passwd_home(&name) else {
            return Err(anyhow!(
                "deploy user {name} not found. Create it first or set RIKU_USER"
            ));
        };
        let group = match primary_group(&name) {
            Ok(group) => group,
            Err(e) => return Err(e),
        };
        Ok(Self {
            name,
            home: PathBuf::from(home),
            group,
        })
    }

    /// The deploy user's riku paths, the ones git push deploys read.
    pub fn riku_paths(&self) -> RikuPaths {
        RikuPaths::from_dirs(self.home.join(".riku"), &self.home)
    }

    /// Give `path` to the deploy user and group.
    pub fn chown(&self, path: &Path) -> Result<()> {
        let owner = format!("{}:{}", self.name, self.group);
        let status = match Command::new("chown").arg(&owner).arg(path).status() {
            Ok(status) => status,
            Err(e) => {
                return Err(anyhow!(
                    "failed to run chown {owner} {}: {e}",
                    path.display()
                ))
            }
        };
        if !status.success() {
            return Err(anyhow!("chown {owner} {} failed", path.display()));
        }
        Ok(())
    }
}
