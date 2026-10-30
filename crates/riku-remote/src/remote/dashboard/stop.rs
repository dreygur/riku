//! Stopping the session's ssh processes without ever SIGKILLing them.
//!
//! ssh may own the terminal for a password, passphrase or host key prompt,
//! and only a signal it handles lets it restore the terminal mode. So a
//! stop waits briefly for ssh to exit on its own, then sends SIGTERM.

use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

use nix::sys::signal::{kill, Signal};
use nix::unistd::Pid;

const POLL: Duration = Duration::from_millis(100);
/// How long an ssh process may take to exit on its own before SIGTERM.
const STOP_GRACE: Duration = Duration::from_secs(3);

/// Waits up to `STOP_GRACE` for `child` to exit, then sends SIGTERM and
/// reaps it.
pub(super) fn stop_child(child: &mut Child) {
    let started = Instant::now();
    while matches!(child.try_wait(), Ok(None)) {
        if started.elapsed() > STOP_GRACE {
            if let Ok(pid) = i32::try_from(child.id()) {
                let _ = kill(Pid::from_raw(pid), Signal::SIGTERM);
            }
            let _ = child.wait();
            return;
        }
        sleep(POLL);
    }
}

/// Runs `ssh <args>` with no output and says whether it succeeded.
pub(super) fn quiet_ssh(args: Vec<String>) -> bool {
    Command::new("ssh")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// The running ssh master. Dropping it asks it to exit with `-O exit`,
/// which also drops its forward, then stops and reaps it.
pub(super) struct Master {
    child: Child,
    exit_args: Vec<String>,
}

impl Master {
    pub(super) fn new(child: Child, exit_args: Vec<String>) -> Self {
        Master { child, exit_args }
    }

    /// The exit status once the master has ended on its own.
    pub(super) fn exited(&mut self) -> Option<ExitStatus> {
        self.child.try_wait().ok().flatten()
    }
}

impl Drop for Master {
    fn drop(&mut self) {
        quiet_ssh(std::mem::take(&mut self.exit_args));
        stop_child(&mut self.child);
    }
}
