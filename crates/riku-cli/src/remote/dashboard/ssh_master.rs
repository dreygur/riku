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
//! takes the token or the password, which only ever travel on stdin.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};

use crate::remote::SshAddress;

/// Longest wait for the master to accept `-O check`.
const READY_TIMEOUT: Duration = Duration::from_secs(10);
const READY_POLL: Duration = Duration::from_millis(100);

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

    fn with_socket(&self, options: &[&str]) -> Vec<String> {
        let mut args = vec!["-S".to_string(), self.path.to_string_lossy().into_owned()];
        args.extend(options.iter().map(|option| option.to_string()));
        args.push("--".to_string());
        args.push(self.admin.as_str().to_string());
        args
    }

    pub(super) fn master_args(&self) -> Vec<String> {
        self.with_socket(&["-M", "-o", "ControlPersist=yes", "-N"])
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
        let spec = format!("{local}:127.0.0.1:{remote}");
        self.with_socket(&["-O", "forward", "-L", &spec])
    }

    pub(super) fn exit_args(&self) -> Vec<String> {
        self.with_socket(&["-O", "exit"])
    }

    fn is_ready(&self) -> bool {
        Command::new("ssh")
            .args(self.check_args())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    }

    /// Starts the master and waits until it accepts clients. ssh may prompt
    /// on the terminal for the admin login while this waits.
    pub(super) fn start(&self) -> Result<Child> {
        let mut master = match Command::new("ssh")
            .args(self.master_args())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .spawn()
        {
            Ok(child) => child,
            Err(e) => return Err(anyhow!("failed to start ssh: {e}")),
        };
        let started = Instant::now();
        while !self.is_ready() {
            if let Ok(Some(status)) = master.try_wait() {
                if !status.success() {
                    return Err(anyhow!("ssh to {} failed ({status})", self.admin.as_str()));
                }
            }
            if started.elapsed() > READY_TIMEOUT {
                let _ = master.kill();
                let _ = master.wait();
                return Err(anyhow!(
                    "ssh to {} was not ready after {} seconds",
                    self.admin.as_str(),
                    READY_TIMEOUT.as_secs()
                ));
            }
            sleep(READY_POLL);
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
            return Err(anyhow!("ssh could not forward port {local} ({status})"));
        }
        Ok(())
    }

    /// Stops the master, which also drops its forward. A master that is
    /// already gone, as after a dropped link, is not an error.
    pub(super) fn stop(&self, mut master: Child) {
        let _ = Command::new("ssh")
            .args(self.exit_args())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        let _ = master.kill();
        let _ = master.wait();
    }
}

#[cfg(test)]
#[path = "ssh_master_tests.rs"]
mod tests;
