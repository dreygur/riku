//! Replacing a supervisor running outside systemd from an older binary,
//! before init starts the `riku` unit.
//!
//! A deploy starts a supervisor with `nohup` when none runs. Such a
//! supervisor holds the pid file lock, so the unit's own would fail its lock
//! on every restart while init reports it started. When it runs another
//! binary than the installed one (see [`super::supervisor_binary`]), it is
//! stopped here first.
//!
//! Security: this runs as the deploy user only (a plain init, or the
//! `init --user-steps` child of a root init), never as root, so the signal
//! can reach only that user's own processes. The pid comes from riku-deploy
//! `supervisor_pid`, which refuses a link, a FIFO or another user's file.

// Project rule 16 forbids `?`, and this lint asks for exactly that rewrite.
#![allow(clippy::question_mark)]

use std::path::Path;
use std::time::Duration;

use crate::config::RikuPaths;
use crate::deploy::supervisor_ctl::running_supervisor_pid;
use crate::deploy::supervisor_stop::stop_supervisor;
use crate::util::{display, ROOT_BINARY};

use super::supervisor_binary::{runs_binary, service_main_pid, UnitScope};

/// How long init waits for the supervisor to stop its workers and exit.
/// Each worker gets its own stop timeout, so this is generous.
const STOP_WAIT: Duration = Duration::from_secs(30);

/// The pid of the supervisor to stop: the one holding the lock (`locked`,
/// always positive), when it is not the unit's `main_pid` and runs another
/// binary than `installed`.
fn to_replace(
    main_pid: Option<u32>,
    locked: Option<i32>,
    proc_root: &Path,
    installed: &Path,
) -> Option<i32> {
    let pid = match locked {
        Some(pid) => pid,
        None => return None,
    };
    let proc_pid = pid as u32;
    if Some(proc_pid) == main_pid || runs_binary(proc_root, proc_pid, installed) {
        return None;
    }
    Some(pid)
}

/// Stops a supervisor running outside the `scope` unit from an older binary,
/// and says so on the rail. Prints nothing when there is none.
pub(super) fn replace_outside_supervisor(paths: &RikuPaths, scope: UnitScope) {
    let locked = running_supervisor_pid(paths);
    let main_pid = service_main_pid(scope);
    let pid = match to_replace(main_pid, locked, Path::new("/proc"), Path::new(ROOT_BINARY)) {
        Some(pid) => pid,
        None => return,
    };
    if stop_supervisor(paths, pid, STOP_WAIT) {
        display::step_done(
            "Supervisor",
            &format!("stopped pid {pid}, an older binary running outside systemd"),
        );
    } else {
        display::warn(&format!(
            "the supervisor running outside systemd (pid {pid}) did not stop; \
             the service cannot start until it does. Stop it, then run init again"
        ));
    }
}

#[cfg(test)]
#[path = "outside_supervisor_tests.rs"]
mod tests;
