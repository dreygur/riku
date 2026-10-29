//! The remote dashboard process of one session, seen through the local ssh
//! client that runs it.
//!
//! The client's stdin stays open for the whole session. Closing it ends the
//! remote `cat`, which kills that session's dashboard and nothing else.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::thread::{sleep, JoinHandle};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};

use super::interrupt;
use super::remote_script::{parse_bound, script_command, SudoMode};
use super::ssh_master::SshControl;
use super::sudo::Secret;

const WAIT_POLL: Duration = Duration::from_millis(200);
/// How long a clean stop may take before the ssh client is killed.
const STOP_GRACE: Duration = Duration::from_secs(5);

/// Why the session's wait ended.
pub(super) enum Ended {
    Interrupted,
    RemoteExited(std::process::ExitStatus),
}

pub(super) struct RemoteDashboard {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: Option<BufReader<ChildStdout>>,
    drain: Option<JoinHandle<()>>,
}

impl RemoteDashboard {
    pub(super) fn spawn(ssh: &SshControl, mode: SudoMode, deploy: &str) -> Result<Self> {
        let mut child = match Command::new("ssh")
            .args(ssh.command_args(&script_command(mode, deploy)))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(e) => return Err(anyhow!("failed to start ssh: {e}")),
        };
        let stdin = child.stdin.take();
        let stdout = child.stdout.take().map(BufReader::new);
        Ok(RemoteDashboard {
            child,
            stdin,
            stdout,
            drain: None,
        })
    }

    /// Writes `payload` to the remote stdin and keeps the pipe open.
    pub(super) fn send(&mut self, payload: &Secret) -> Result<()> {
        let stdin = match self.stdin.as_mut() {
            Some(stdin) => stdin,
            None => return Err(anyhow!("the remote session has no stdin")),
        };
        match stdin
            .write_all(payload.as_bytes())
            .and_then(|()| stdin.flush())
        {
            Ok(()) => Ok(()),
            Err(e) => Err(anyhow!("could not send the session token: {e}")),
        }
    }

    /// Reads remote stdout up to the bound port line, passing every other
    /// line to stderr, then keeps passing later lines on a thread so the
    /// remote side never blocks on a full pipe.
    pub(super) fn bound_port(&mut self) -> Result<u16> {
        let mut stdout = match self.stdout.take() {
            Some(stdout) => stdout,
            None => return Err(anyhow!("the remote session has no stdout")),
        };
        let mut line = String::new();
        loop {
            line.clear();
            match stdout.read_line(&mut line) {
                Ok(0) => {
                    return Err(anyhow!(
                        "the remote dashboard stopped before reporting its port"
                    ))
                }
                Ok(_) => {}
                Err(e) => return Err(anyhow!("could not read from the remote dashboard: {e}")),
            }
            if let Some(port) = parse_bound(&line) {
                self.drain = Some(std::thread::spawn(move || pass_to_stderr(stdout)));
                return Ok(port);
            }
            eprint!("{line}");
        }
    }

    /// Blocks until Ctrl-C or the remote side ends.
    pub(super) fn wait(&mut self) -> Result<Ended> {
        loop {
            if interrupt::requested() {
                return Ok(Ended::Interrupted);
            }
            match self.child.try_wait() {
                Ok(Some(status)) => return Ok(Ended::RemoteExited(status)),
                Ok(None) => sleep(WAIT_POLL),
                Err(e) => return Err(anyhow!("could not wait for ssh: {e}")),
            }
        }
    }

    /// Closes stdin so the remote dashboard stops, and kills the ssh client
    /// if that takes longer than `STOP_GRACE`, as on a dead link.
    pub(super) fn finish(mut self) {
        drop(self.stdin.take());
        let started = Instant::now();
        while matches!(self.child.try_wait(), Ok(None)) {
            if started.elapsed() > STOP_GRACE {
                let _ = self.child.kill();
                let _ = self.child.wait();
                break;
            }
            sleep(WAIT_POLL);
        }
        if let Some(drain) = self.drain.take() {
            let _ = drain.join();
        }
    }
}

fn pass_to_stderr(stdout: BufReader<ChildStdout>) {
    for line in stdout.lines().map_while(Result::ok) {
        eprintln!("{line}");
    }
}
