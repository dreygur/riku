//! The remote dashboard process of one session, seen through the local ssh
//! client that runs it, and the handshake that starts it.
//!
//! The client's stdin stays open for the whole session. Nothing is written
//! to it except the password after sudo's prompt and the token after the
//! ready marker. Closing it ends the remote `cat`, which kills that
//! session's dashboard and nothing else.

use std::io::Write;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError};
use std::thread::{sleep, JoinHandle};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};

use super::events::{read_stderr, read_stdout, Event};
use super::handshake::{Action, Handshake};
use super::remote_script::script_command;
use super::ssh_master::SshControl;
use super::stop::stop_child;
use super::{interrupt, sudo};

const POLL: Duration = Duration::from_millis(200);

fn show(action: &Action) {
    match action {
        Action::ShowStderr(bytes) => {
            let _ = std::io::stderr().write_all(bytes);
        }
        Action::ShowStdout(line) => eprintln!("{line}"),
        _ => {}
    }
}

pub(super) struct RemoteDashboard {
    child: Child,
    stdin: Option<ChildStdin>,
    events: Receiver<Event>,
    readers: Vec<JoinHandle<()>>,
    handshake: Handshake,
}

impl RemoteDashboard {
    pub(super) fn spawn(ssh: &SshControl, deploy: &str) -> Result<Self> {
        let mut child = match Command::new("ssh")
            .args(ssh.command_args(&script_command(deploy)))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(e) => return Err(anyhow!("failed to start ssh: {e}")),
        };
        let (sender, events) = channel();
        let mut readers = Vec::with_capacity(2);
        if let Some(stdout) = child.stdout.take() {
            readers.push(read_stdout(stdout, sender.clone()));
        }
        if let Some(stderr) = child.stderr.take() {
            readers.push(read_stderr(stderr, sender));
        }
        Ok(RemoteDashboard {
            stdin: child.stdin.take(),
            child,
            events,
            readers,
            handshake: Handshake::new(),
        })
    }

    /// The handshake's response to one event. `None` once stdout closed.
    fn react(&mut self, event: Event) -> Option<Vec<Action>> {
        match event {
            Event::Stderr(bytes) => Some(self.handshake.stderr(&bytes)),
            Event::StderrClosed => Some(self.handshake.stderr_closed()),
            Event::StdoutLine(line) => Some(self.handshake.stdout_line(&line)),
            Event::StdoutClosed => None,
        }
    }

    fn stdin(&mut self) -> Result<&mut ChildStdin> {
        match self.stdin.as_mut() {
            Some(stdin) => Ok(stdin),
            None => Err(anyhow!("the remote session has no stdin")),
        }
    }

    #[allow(clippy::question_mark)]
    fn send_password(&mut self, admin_user: &str) -> Result<()> {
        let password = match sudo::prompt_password(admin_user) {
            Ok(Some(password)) => password,
            Ok(None) => return Err(anyhow!("no password given")),
            Err(e) => return Err(e),
        };
        let stdin = match self.stdin() {
            Ok(stdin) => stdin,
            Err(e) => return Err(e),
        };
        match password.write_line(stdin) {
            Ok(()) => Ok(()),
            Err(e) => Err(anyhow!("could not send the password: {e}")),
        }
    }

    #[allow(clippy::question_mark)]
    fn send_token(&mut self, token: &str) -> Result<()> {
        let stdin = match self.stdin() {
            Ok(stdin) => stdin,
            Err(e) => return Err(e),
        };
        let sent = stdin
            .write_all(token.as_bytes())
            .and_then(|()| stdin.write_all(b"\n"))
            .and_then(|()| stdin.flush());
        match sent {
            Ok(()) => Ok(()),
            Err(e) => Err(anyhow!("could not send the session token: {e}")),
        }
    }

    /// The actions for the next event before `deadline`, or `None` when no
    /// event came within one poll.
    fn next_actions(&mut self, deadline: Instant) -> Result<Option<Vec<Action>>> {
        if interrupt::requested() {
            return Err(anyhow!("interrupted"));
        }
        let stage = self.handshake.stage();
        if Instant::now() > deadline {
            return Err(anyhow!("{}", stage.stalled()));
        }
        let event = match self.events.recv_timeout(POLL) {
            Ok(event) => event,
            Err(RecvTimeoutError::Timeout) => return Ok(None),
            Err(RecvTimeoutError::Disconnected) => return Err(anyhow!("{}", stage.ended())),
        };
        match self.react(event) {
            Some(actions) => Ok(Some(actions)),
            None => Err(anyhow!("{}", stage.ended())),
        }
    }

    /// Runs the handshake and returns the port the remote dashboard bound.
    /// Each stage gets its own deadline, restarted when riku answers sudo
    /// or sends the token, so time spent typing the password never counts.
    #[allow(clippy::question_mark)]
    pub(super) fn start(&mut self, admin_user: &str, token: &str) -> Result<u16> {
        let mut deadline = Instant::now() + self.handshake.stage().deadline();
        loop {
            let actions = match self.next_actions(deadline) {
                Ok(Some(actions)) => actions,
                Ok(None) => continue,
                Err(e) => return Err(e),
            };
            for action in actions {
                let answered = match action {
                    Action::AskPassword => self.send_password(admin_user),
                    Action::SendToken => self.send_token(token),
                    Action::RejectPassword => {
                        drop(self.stdin.take());
                        return Err(anyhow!("sudo on the server rejected the password"));
                    }
                    Action::Bound(port) => return Ok(port),
                    output => {
                        show(&output);
                        continue;
                    }
                };
                match answered {
                    Ok(()) => deadline = Instant::now() + self.handshake.stage().deadline(),
                    Err(e) => return Err(e),
                }
            }
        }
    }

    /// Passes remote output through until a stop signal (`Ok`) or the remote
    /// side ends (`Err`).
    pub(super) fn wait(&mut self) -> Result<()> {
        loop {
            if interrupt::requested() {
                return Ok(());
            }
            match self.child.try_wait() {
                Ok(Some(status)) => return Err(anyhow!("the remote dashboard ended ({status})")),
                Ok(None) => {}
                Err(e) => return Err(anyhow!("could not wait for ssh: {e}")),
            }
            match self.events.recv_timeout(POLL) {
                Ok(event) => self.react(event).iter().flatten().for_each(show),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => sleep(POLL),
            }
        }
    }

    /// Closes stdin so the remote dashboard stops, and stops the ssh client.
    /// Returns the reader threads, which may only end once the master is
    /// gone, since a shared connection's master holds the client's pipes.
    pub(super) fn finish(mut self) -> Vec<JoinHandle<()>> {
        drop(self.stdin.take());
        stop_child(&mut self.child);
        self.readers
    }
}
