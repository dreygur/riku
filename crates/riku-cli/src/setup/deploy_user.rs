//! The account apps deploy as, seen from a root `riku init`.
//!
//! Root init runs with `HOME=/root`, but git push deploys run as this user
//! and read `~/.riku` from its home. Everything deploys must see (the
//! binary, `~/.riku`, the git hook, the Caddy site directory) belongs in
//! this user's tree.
//!
//! Security: the deploy user owns that tree, so it can plant symlinks in it
//! (`~/.riku/ROUTER -> /etc/shadow`). Root must never create, write, chmod or
//! chown a path there: a followed symlink would hand the target to the
//! deploy user. Every such change runs as a child process with the deploy
//! user's uid and gid (supplementary groups dropped), so it can only touch
//! what the deploy user could already touch. Commands get a fixed argv and
//! no shell.

use std::env;
use std::ffi::OsStr;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{anyhow, Result};
use nix::unistd::{setsid, Gid, Group, Uid, User};

use crate::config::RikuPaths;

use super::default_branch::BRANCH_ENV;

const DEFAULT_USER: &str = "riku";
/// Root's environment variables a deploy user child may see.
const PASSED_ENV: [&str; 4] = ["PATH", "TERM", "RIKU_ROUTER", BRANCH_ENV];

/// The deploy account, resolved from the passwd and group databases.
pub struct DeployUser {
    pub name: String,
    pub home: PathBuf,
    /// Primary group name, for adding other users to it.
    pub group: String,
    uid: Uid,
    gid: Gid,
}

/// Whether this process runs with root's effective uid. `$USER` is not used:
/// `sudo -E` keeps the caller's value and a stale one can name any account.
pub(crate) fn running_as_root() -> bool {
    Uid::effective().is_root()
}

/// The `RIKU_USER` login name, `riku` when unset.
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

/// `bytes` as text with every control character except newline and tab
/// removed. Relayed child output reaches root's terminal, where an escape
/// sequence could set the clipboard, retitle the window or make the
/// terminal answer back as input.
fn printable(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .chars()
        .filter(|c| !c.is_control() || matches!(c, '\n' | '\t'))
        .collect()
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

    /// A command that runs as the deploy user from its home.
    ///
    /// The child starts in a new session with no controlling terminal and an
    /// allowlisted environment. Holding root's terminal would let deploy
    /// owned code push keystrokes into root's shell (TIOCSTI) or read what
    /// root types after init returns. Callers also pass no terminal fds: the
    /// helpers below use null stdin and piped output. std drops
    /// supplementary groups when a root process sets the uid.
    fn command(&self, program: impl AsRef<OsStr>) -> Command {
        let mut command = Command::new(program);
        command
            .env_clear()
            .envs(env::vars_os().filter(|(key, _)| PASSED_ENV.iter().any(|k| key == k)))
            .env("HOME", &self.home)
            .env("USER", &self.name)
            .env("LOGNAME", &self.name)
            .env("NO_COLOR", "1")
            .uid(self.uid.as_raw())
            .gid(self.gid.as_raw())
            .current_dir(&self.home)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // SAFETY: setsid is async-signal-safe and touches no memory shared
        // with the parent, as `pre_exec` requires.
        unsafe {
            command.pre_exec(|| match setsid() {
                Ok(_) => Ok(()),
                Err(e) => Err(std::io::Error::from(e)),
            });
        }
        command
    }

    /// Run `program` as the deploy user. Returns its stdout.
    fn run_as(&self, program: &str, args: &[&str]) -> Result<String> {
        let command = format!("{program} {}", args.join(" "));
        let output = match self.command(program).args(args).output() {
            Ok(output) => output,
            Err(e) => return Err(anyhow!("failed to run {command} as {}: {e}", self.name)),
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
        self.run_as("mkdir", &["-p", &dir.to_string_lossy()])
            .map(drop)
    }

    /// Change `path`'s mode as the deploy user (`mode` in chmod syntax).
    pub fn chmod(&self, mode: &str, path: &Path) -> Result<()> {
        self.run_as("chmod", &[mode, &path.to_string_lossy()])
            .map(drop)
    }

    /// Read `path` as the deploy user.
    pub fn read_file(&self, path: &Path) -> Result<String> {
        self.run_as("cat", &[&path.to_string_lossy()])
    }

    /// Run `program` as the deploy user and replay its output on root's
    /// terminal once it exits, failure included, stripped of control
    /// characters (see [`printable`]).
    pub fn run_relayed(&self, program: &Path, args: &[&str]) -> Result<()> {
        let shown = format!("{} {}", program.display(), args.join(" "));
        let output = match self.command(program).args(args).output() {
            Ok(output) => output,
            Err(e) => return Err(anyhow!("failed to run {shown} as {}: {e}", self.name)),
        };
        print!("{}", printable(&output.stdout));
        eprint!("{}", printable(&output.stderr));
        if !output.status.success() {
            return Err(anyhow!(
                "{shown} as {} failed: {}",
                self.name,
                output.status
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "deploy_user_tests.rs"]
mod tests;
