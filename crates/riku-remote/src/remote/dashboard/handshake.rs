//! Turns what the remote session prints into what the local side does next.
//!
//! Pure, so every ordering of prompt, marker and port line can be tested
//! without a host. The token is only ever requested after `READY_MARKER`,
//! and a second sudo prompt aborts so a wrong password costs one attempt.

use std::time::Duration;

use super::remote_script::{parse_bound, READY_MARKER, SUDO_PROMPT};

/// Longest wait for sudo to prompt or let the inner shell run.
const SUDO_DEADLINE: Duration = Duration::from_secs(30);
/// Longest wait for the bound port line once the token is sent.
const BOUND_DEADLINE: Duration = Duration::from_secs(30);

/// Something the session must do in response to remote output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Action {
    /// Remote stderr text to pass to the local stderr.
    ShowStderr(Vec<u8>),
    /// A remote stdout line, without its newline, to pass on.
    ShowStdout(String),
    AskPassword,
    SendToken,
    /// sudo prompted again, so it rejected the password.
    RejectPassword,
    /// The remote dashboard listens on this server port.
    Bound(u16),
}

/// Where the handshake is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Stage {
    /// Waiting for sudo to prompt or to let the inner shell run.
    Sudo,
    /// Token sent, waiting for the bound port line.
    Starting,
    /// Port known; remote output is only passed through.
    Running,
}

impl Stage {
    /// Longest wait in this stage before the session gives up.
    pub(super) fn deadline(self) -> Duration {
        match self {
            Stage::Sudo => SUDO_DEADLINE,
            Stage::Starting | Stage::Running => BOUND_DEADLINE,
        }
    }

    /// Why the session stopped when this stage ran past its deadline.
    pub(super) fn stalled(self) -> String {
        match self {
            Stage::Sudo => format!(
                "sudo on the server neither asked for a password nor started the dashboard within {} seconds",
                SUDO_DEADLINE.as_secs()
            ),
            Stage::Starting | Stage::Running => format!(
                "the remote dashboard did not report its port within {} seconds; is riku on the server up to date?",
                BOUND_DEADLINE.as_secs()
            ),
        }
    }

    /// Why the session stopped when the remote side closed in this stage.
    pub(super) fn ended(self) -> &'static str {
        match self {
            Stage::Sudo => "the remote session ended before sudo let it run (see above)",
            Stage::Starting | Stage::Running => {
                "the remote session ended before the dashboard reported its port"
            }
        }
    }
}

pub(super) struct Handshake {
    stage: Stage,
    /// Tail of stderr that may be the start of a prompt split across reads.
    held: Vec<u8>,
    prompts: usize,
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// Length of the longest tail of `bytes` that `prompt` starts with.
fn prompt_prefix_len(bytes: &[u8], prompt: &[u8]) -> usize {
    (1..prompt.len().min(bytes.len() + 1))
        .rev()
        .find(|&n| bytes.ends_with(&prompt[..n]))
        .unwrap_or(0)
}

impl Handshake {
    pub(super) fn new() -> Self {
        Handshake {
            stage: Stage::Sudo,
            held: Vec::new(),
            prompts: 0,
        }
    }

    pub(super) fn stage(&self) -> Stage {
        self.stage
    }

    fn release_held(&mut self) -> Vec<Action> {
        if self.held.is_empty() {
            return Vec::new();
        }
        vec![Action::ShowStderr(std::mem::take(&mut self.held))]
    }

    pub(super) fn stderr(&mut self, bytes: &[u8]) -> Vec<Action> {
        self.held.extend_from_slice(bytes);
        if self.stage != Stage::Sudo {
            return self.release_held();
        }
        let prompt = SUDO_PROMPT.as_bytes();
        let mut actions = Vec::new();
        while let Some(at) = find(&self.held, prompt) {
            let before: Vec<u8> = self.held.drain(..at + prompt.len()).take(at).collect();
            if !before.is_empty() {
                actions.push(Action::ShowStderr(before));
            }
            self.prompts += 1;
            if self.prompts > 1 {
                actions.push(Action::RejectPassword);
                return actions;
            }
            actions.push(Action::AskPassword);
        }
        let keep = prompt_prefix_len(&self.held, prompt);
        let show: Vec<u8> = self.held.drain(..self.held.len() - keep).collect();
        if !show.is_empty() {
            actions.push(Action::ShowStderr(show));
        }
        actions
    }

    /// Remote stderr ended: whatever was held cannot be a prompt any more.
    pub(super) fn stderr_closed(&mut self) -> Vec<Action> {
        self.release_held()
    }

    pub(super) fn stdout_line(&mut self, line: &str) -> Vec<Action> {
        let line = line.trim_end_matches(['\n', '\r']);
        match self.stage {
            Stage::Sudo if line == READY_MARKER => {
                self.stage = Stage::Starting;
                let mut actions = self.release_held();
                actions.push(Action::SendToken);
                actions
            }
            Stage::Starting => match parse_bound(line) {
                Some(port) => {
                    self.stage = Stage::Running;
                    vec![Action::Bound(port)]
                }
                None => vec![Action::ShowStdout(line.to_string())],
            },
            _ => vec![Action::ShowStdout(line.to_string())],
        }
    }
}

#[cfg(test)]
#[path = "handshake_tests.rs"]
mod tests;
