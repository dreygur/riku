//! Stopping a supervisor running outside systemd before init starts the
//! `riku` unit.
//!
//! A deploy starts a supervisor with `nohup` when none runs. Such a
//! supervisor holds the pid file lock, so the unit's own would fail its lock
//! on every restart (`Restart=always`) while init reports it started. Any
//! supervisor holding the lock that is not the unit's main process is
//! stopped here first, whatever binary it runs and whether or not the unit
//! is running. One that does not stop fails init instead of leaving the unit
//! looping on the lock.
//!
//! Security: this runs as the deploy user only (a plain init, or the
//! `init --user-steps` child of a root init), never as root, so the signal
//! can reach only that user's own processes. The pid comes from riku-deploy
//! `supervisor_pid`, which refuses a link, a FIFO or another user's file.

// Project rule 16 forbids `?`, and this lint asks for exactly that rewrite.
#![allow(clippy::question_mark)]

use std::time::Duration;

use anyhow::{anyhow, Result};

use crate::config::RikuPaths;
use crate::deploy::supervisor_ctl::running_supervisor_pid;
use crate::deploy::supervisor_stop::stop_supervisor;
use crate::util::display;

use super::supervisor_binary::{service_main_pid, UnitScope};

/// How long init waits for the supervisor to stop its workers and exit.
/// Each worker gets its own stop timeout, so this is generous.
const STOP_WAIT: Duration = Duration::from_secs(30);

/// The pid of the supervisor to stop: the one holding the lock (`locked`,
/// always positive), unless it is the unit's `main_pid`.
fn to_replace(main_pid: Option<u32>, locked: Option<i32>) -> Option<i32> {
    match locked {
        Some(pid) if Some(pid as u32) != main_pid => Some(pid),
        _ => None,
    }
}

/// Stops a supervisor running outside the `scope` unit and says so on the
/// rail. Prints nothing when there is none. An error when it does not stop
/// within [`STOP_WAIT`], since the unit cannot run while it holds the lock.
pub(super) fn replace_outside_supervisor(paths: &RikuPaths, scope: UnitScope) -> Result<()> {
    let locked = running_supervisor_pid(paths);
    let pid = match to_replace(service_main_pid(scope), locked) {
        Some(pid) => pid,
        None => return Ok(()),
    };
    if !stop_supervisor(paths, pid, STOP_WAIT) {
        return Err(anyhow!(
            "the supervisor running outside systemd (pid {pid}) did not stop within {} s; \
             the riku service cannot start while it holds the lock. Stop it (kill {pid}), \
             then run init again",
            STOP_WAIT.as_secs()
        ));
    }
    display::step_done(
        "Supervisor",
        &format!("stopped pid {pid}, running outside systemd, so the service takes over"),
    );
    Ok(())
}

#[cfg(test)]
#[path = "outside_supervisor_tests.rs"]
mod tests;
