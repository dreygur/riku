//! One OpenSSH shared connection per dashboard session, and the `ssh`
//! argument lists for every step that runs over it.
//!
//! Starting, checking, forwarding over and stopping the master need a
//! reachable host, so only the argument builders are unit tested.
//!
//! Security model: the control socket lives in a private temp dir made for
//! this session, so no other local user can attach to the connection. The
//! admin address is a validated `SshAddress` and follows `--`. The remote
//! command is a single argument quoted with `shlex::try_join`. No builder
//! takes the token or the password, which only ever travel on stdin. The
//! local end of the forward binds `127.0.0.1` explicitly, and a failed
//! forward ends the session, so a process already on that port never gets
//! the token.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};

use super::interrupt;
use super::stop::{quiet_ssh, Master};
use crate::remote::SshAddress;

/// Longest wait for the master to accept clients. Covers typing an ssh
/// password or passphrase; the TCP connect itself is bounded by
/// `ConnectTimeout`.
const CONNECT_DEADLINE: Duration = Duration::from_secs(60);
const POLL: Duration = Duration::from_millis(100);

/// ssh expands `%C` to a hash of the connection, which keeps the socket path
/// short and unique.
const CONTROL_NAME: &str = "%C";

/// Control socket path inside the session's private `dir`.
pub(super) fn control_path(dir: &Path) -> PathBuf {
    dir.join(CONTROL_NAME)
}

/// The shared connection to the server's admin login.
pub(super) struct SshControl<'a> {
    path: &'a Path,
    admin: &'a SshAddress,
}

impl<'a> SshControl<'a> {
    pub(super) fn new(path: &'a Path, admin: &'a SshAddress) -> Self {
        SshControl { path, admin }
    }

    /// The admin address as typed, for messages.
    pub(super) fn target(&self) -> &str {
        self.admin.as_str()
    }

    fn with_socket(&self, options: &[&str]) -> Vec<String> {
        let mut args = vec!["-S".to_string(), self.path.to_string_lossy().into_owned()];
        args.extend(options.iter().map(|option| option.to_string()));
        args.push("--".to_string());
        args.push(self.admin.destination());
        args
    }

    /// The master stays a child of riku (`ControlPersist=no`), and keepalives
    /// end it within about 45 seconds of a dead link.
    pub(super) fn master_args(&self) -> Vec<String> {
        self.with_socket(&[
            "-M",
            "-o",
            "ControlPersist=no",
            "-o",
            "ConnectTimeout=10",
            "-o",
            "ServerAliveInterval=15",
            "-o",
            "ServerAliveCountMax=3",
            "-N",
        ])
    }

    pub(super) fn check_args(&self) -> Vec<String> {
        self.with_socket(&["-O", "check"])
    }

    /// Runs `words` on the server as one quoted remote command.
    pub(super) fn command_args(&self, words: &[String]) -> Vec<String> {
        // The words are the fixed script and a validated user name, so they
        // never hold the NUL that would make quoting fail.
        let remote = shlex::try_join(words.iter().map(String::as_str))
            .expect("dashboard remote words never contain NUL");
        let mut args = self.with_socket(&[]);
        args.push(remote);
        args
    }

    pub(super) fn forward_args(&self, local: u16, remote: u16) -> Vec<String> {
        let spec = format!("127.0.0.1:{local}:127.0.0.1:{remote}");
        self.with_socket(&["-O", "forward", "-L", &spec])
    }

    pub(super) fn exit_args(&self) -> Vec<String> {
        self.with_socket(&["-O", "exit"])
    }

    /// Starts the master and waits until it accepts clients. ssh may prompt
    /// on the terminal for the admin login while this waits.
    pub(super) fn start(&self) -> Result<Master> {
        let child = match Command::new("ssh")
            .args(self.master_args())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .spawn()
        {
            Ok(child) => child,
            Err(e) => return Err(anyhow!("failed to start ssh: {e}")),
        };
        let mut master = Master::new(child, self.exit_args());
        let started = Instant::now();
        while !quiet_ssh(self.check_args()) {
            if interrupt::requested() {
                return Err(anyhow!("interrupted"));
            }
            if let Some(status) = master.exited() {
                return Err(anyhow!("ssh to {} failed ({status})", self.admin.as_str()));
            }
            if started.elapsed() > CONNECT_DEADLINE {
                return Err(anyhow!(
                    "ssh to {} did not connect within {} seconds",
                    self.admin.as_str(),
                    CONNECT_DEADLINE.as_secs()
                ));
            }
            sleep(POLL);
        }
        Ok(master)
    }

    /// Forwards local `127.0.0.1:<local>` to the server's `127.0.0.1:<remote>`.
    pub(super) fn add_forward(&self, local: u16, remote: u16) -> Result<()> {
        let status = match Command::new("ssh")
            .args(self.forward_args(local, remote))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .status()
        {
            Ok(status) => status,
            Err(e) => return Err(anyhow!("failed to start ssh: {e}")),
        };
        if !status.success() {
            return Err(anyhow!(
                "ssh could not forward 127.0.0.1:{local} ({status}); stopping so the token goes nowhere else"
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "ssh_master_tests.rs"]
mod tests;
