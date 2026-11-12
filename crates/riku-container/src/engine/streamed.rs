//! Engine calls whose output the operator watches as they run.
//!
//! Pulls and builds take minutes and print progress, and a release
//! container's logs explain why it failed. [`Engine::run_teed`] reads the
//! engine's stdout and stderr line by line and puts every line on the output
//! rail (`│  line`) on stdout as it arrives, as runtime plugin output is, so
//! it reaches the `git push` terminal live and in order, and keeps the last
//! lines in an [`OutputTail`]. The call is still stopped at its own limit
//! (see [`super::bounded`]). The engine runs in its own process group, so a
//! Ctrl-C at the terminal reaches riku but not the engine client.

use std::ffi::OsString;
use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::thread::{sleep, spawn, JoinHandle};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use nix::sys::signal::{killpg, Signal};
use nix::unistd::Pid;
use riku_util::display::{self, for_each_line, OutputTail};

use super::bounded::{spawn_in_group, wait_bounded};
use super::Engine;

/// How long the readers may keep draining once the engine client exited. A
/// helper outside the group could hold a pipe open; the readers are then
/// left behind rather than blocking the deploy.
const DRAIN_GRACE: Duration = Duration::from_secs(2);

fn command_name(args: &[OsString]) -> String {
    args.first()
        .map(|a| a.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn tee_lines(pipe: impl Read + Send + 'static, recent: &OutputTail) -> JoinHandle<()> {
    let recent = recent.clone();
    spawn(move || {
        for_each_line(pipe, |line| {
            display::detail(line);
            recent.push(line);
        })
    })
}

fn tee_child(child: &mut Child, recent: &OutputTail) -> Vec<JoinHandle<()>> {
    let mut readers = Vec::new();
    if let Some(stdout) = child.stdout.take() {
        readers.push(tee_lines(stdout, recent));
    }
    if let Some(stderr) = child.stderr.take() {
        readers.push(tee_lines(stderr, recent));
    }
    readers
}

fn finish_readers(readers: Vec<JoinHandle<()>>, pgid: Pid) {
    let deadline = Instant::now() + DRAIN_GRACE;
    while Instant::now() < deadline && !readers.iter().all(JoinHandle::is_finished) {
        sleep(Duration::from_millis(10));
    }
    if !readers.iter().all(JoinHandle::is_finished) {
        let _ = killpg(pgid, Signal::SIGKILL);
        return;
    }
    for reader in readers {
        let _ = reader.join();
    }
}

impl Engine {
    /// Run the engine with `args` within `limit`, putting each line of its
    /// stdout and stderr on the output rail and into `recent`. A failure
    /// reports the exit status; the engine's reason is already on the rail.
    pub(super) fn run_teed(
        &self,
        args: &[OsString],
        subject: &str,
        limit: Duration,
        recent: &OutputTail,
    ) -> Result<()> {
        let command = command_name(args);
        let mut child = match spawn_in_group(
            Command::new(self.program())
                .args(args)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped()),
        ) {
            Ok(child) => child,
            Err(e) => {
                return Err(anyhow!(
                    "cannot run '{command}' on the container engine: {e}"
                ))
            }
        };
        let pgid = Pid::from_raw(child.id() as i32);
        let readers = tee_child(&mut child, recent);
        let waited = wait_bounded(&mut child, &command, limit);
        finish_readers(readers, pgid);
        let status = match waited {
            Ok(status) => status,
            Err(e) => return Err(anyhow!("cannot {command} {subject}: {e}")),
        };
        if !status.success() {
            return Err(anyhow!(
                "container engine '{command}' failed for {subject} with {status}, see the output above"
            ));
        }
        Ok(())
    }
}
