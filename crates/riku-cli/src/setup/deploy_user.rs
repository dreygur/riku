//! The account apps deploy as, seen from a root `riku init`.
//!
//! Root init runs with `HOME=/root`, but git push deploys run as this user
//! and read `~/.riku` from its home. Anything deploys must see (the
//! `ROUTER` file, the Caddy site directory) goes into this user's tree.
//!
//! Security: the deploy user owns that tree, so it can plant symlinks in it
//! (`~/.riku/ROUTER -> /etc/shadow`). Root must never create, write, chmod or
//! chown a path there: a followed symlink would hand the target to the
//! deploy user. Every such change runs as a child process with the deploy
//! user's uid and gid (supplementary groups dropped), so it can only touch
//! what the deploy user could already touch. Commands get a fixed argv, no
//! shell, and file contents go through stdin.

use std::io::Write;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{anyhow, Result};
use nix::unistd::{Gid, Group, Uid, User};

use crate::config::RikuPaths;

const DEFAULT_USER: &str = "deploy";

/// The deploy account, resolved from the passwd and group databases.
pub struct DeployUser {
    pub name: String,
    pub home: PathBuf,
    /// Primary group name, for adding other users to it.
    pub group: String,
    uid: Uid,
    gid: Gid,
}

/// The `RIKU_USER` login name, `deploy` when unset.
pub fn deploy_user_name() -> String {
    std::env::var("RIKU_USER").unwrap_or_else(|_| DEFAULT_USER.to_string())
}

/// `user`'s home directory from the passwd database.
pub fn passwd_home(user: &str) -> Option<String> {
    match User::from_name(user) {
        Ok(Some(entry)) => Some(entry.dir.to_string_lossy().into_owned()),
        _ => None,
    }
}

fn group_name(gid: Gid) -> Result<String> {
    match Group::from_gid(gid) {
        Ok(Some(group)) => Ok(group.name),
        Ok(None) => Err(anyhow!("no group with gid {gid}")),
        Err(e) => Err(anyhow!("failed to look up gid {gid}: {e}")),
    }
}

impl DeployUser {
    /// Look up the `RIKU_USER` account. Unlike the systemd unit writer, this
    /// does not guess a home: acting on a guessed path is worse than failing.
    #[allow(clippy::question_mark)]
    pub fn lookup() -> Result<Self> {
        let name = deploy_user_name();
        let entry = match User::from_name(&name) {
            Ok(Some(entry)) => entry,
            Ok(None) => {
                return Err(anyhow!(
                    "deploy user {name} not found. Create it first or set RIKU_USER"
                ))
            }
            Err(e) => return Err(anyhow!("failed to look up user {name}: {e}")),
        };
        let group = match group_name(entry.gid) {
            Ok(group) => group,
            Err(e) => return Err(e),
        };
        Ok(Self {
            name,
            home: entry.dir,
            group,
            uid: entry.uid,
            gid: entry.gid,
        })
    }

    /// The deploy user's riku paths, the ones git push deploys read.
    pub fn riku_paths(&self) -> RikuPaths {
        RikuPaths::from_dirs(self.home.join(".riku"), &self.home)
    }

    /// Run `program` as the deploy user, feeding `input` on stdin. Returns
    /// its stdout.
    fn run_as(&self, program: &str, args: &[&str], input: &str) -> Result<String> {
        let command = format!("{program} {}", args.join(" "));
        let mut child = match Command::new(program)
            .args(args)
            .uid(self.uid.as_raw())
            .gid(self.gid.as_raw())
            .current_dir(&self.home)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(e) => return Err(anyhow!("failed to run {command} as {}: {e}", self.name)),
        };
        if let Some(mut stdin) = child.stdin.take() {
            if let Err(e) = stdin.write_all(input.as_bytes()) {
                return Err(anyhow!("failed to feed {command}: {e}"));
            }
        }
        let output = match child.wait_with_output() {
            Ok(output) => output,
            Err(e) => return Err(anyhow!("failed to wait for {command}: {e}")),
        };
        if !output.status.success() {
            return Err(anyhow!(
                "{command} as {} failed: {}",
                self.name,
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    /// Create `dir` and its parents as the deploy user.
    pub fn create_dir(&self, dir: &Path) -> Result<()> {
        self.run_as("mkdir", &["-p", &dir.to_string_lossy()], "")
            .map(drop)
    }

    /// Change `path`'s mode as the deploy user (`mode` in chmod syntax).
    pub fn chmod(&self, mode: &str, path: &Path) -> Result<()> {
        self.run_as("chmod", &[mode, &path.to_string_lossy()], "")
            .map(drop)
    }

    /// Replace `path`'s contents as the deploy user.
    pub fn write_file(&self, path: &Path, contents: &str) -> Result<()> {
        self.run_as("tee", &[&path.to_string_lossy()], contents)
            .map(drop)
    }

    /// Read `path` as the deploy user.
    pub fn read_file(&self, path: &Path) -> Result<String> {
        self.run_as("cat", &[&path.to_string_lossy()], "")
    }
}

#[cfg(test)]
#[path = "deploy_user_tests.rs"]
mod tests;
