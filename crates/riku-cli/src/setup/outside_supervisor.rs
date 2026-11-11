//! Stopping a supervisor running outside systemd before init starts the
//! `riku` unit.
//!
//! A deploy starts a supervisor with `nohup` when none runs. Such a
//! supervisor holds the pid file lock, so the unit's own would fail its lock
//! on every restart (`Restart=always`) while init reports it started. Any
//! supervisor holding the lock that is not the main process of the system
//! or the user `riku` unit is stopped here first, whatever binary it runs
//! and whether or not the unit is running. One that does not stop fails init
//! instead of leaving the unit looping on the lock. When the other scope's
//! unit holds the lock, init stops with an error naming what to run instead.
//!
//! Security: this runs as the deploy user only (a plain init, or the
//! `init --user-steps` child of a root init), never as root, so the signal
//! can reach only that user's own processes. The pid comes from riku-deploy
//! `supervisor_pid`, which refuses a link, a FIFO or another user's file.

// Project rule 16 forbids `?`, and this lint asks for exactly that rewrite.
#![allow(clippy::question_mark)]

use std::path::Path;
use std::time::Duration;

use anyhow::{anyhow, Result};

use crate::config::RikuPaths;
use crate::deploy::supervisor_stop::{seen_supervisor, stop_supervisor};
use crate::util::display;

use super::supervisor_binary::{service_main_pid, UnitScope};

/// How long init waits for the supervisor to stop its workers and exit.
/// Each worker gets its own stop timeout, so this is generous.
const STOP_WAIT: Duration = Duration::from_secs(30);

/// The main pids of the `riku` unit in the scope init sets up and in the
/// other one.
#[derive(Debug, Clone, Copy)]
struct MainPids {
    own: Option<u32>,
    other: Option<u32>,
}

/// Who holds the pid file lock.
#[derive(Debug, PartialEq, Eq)]
enum Holder {
    /// The unit init is setting up; systemd restarts it when needed.
    OwnUnit,
    /// The `riku` unit of the other scope, which must not be signalled.
    OtherUnit,
    /// A supervisor outside both units, with its pid.
    Outside(i32),
}

fn holder(units: MainPids, locked: i32) -> Holder {
    let locked_pid = Some(locked as u32);
    if locked_pid == units.own {
        Holder::OwnUnit
    } else if locked_pid == units.other {
        Holder::OtherUnit
    } else {
        Holder::Outside(locked)
    }
}

/// The refusal when the other scope's unit runs the supervisor.
fn other_unit_error(scope: UnitScope) -> anyhow::Error {
    match scope {
        UnitScope::User => {
            anyhow!("riku runs as the system unit riku.service; re-run as root: sudo riku init")
        }
        UnitScope::System => anyhow!(
            "riku runs as the deploy user's user unit riku.service; as that user run \
             systemctl --user disable --now riku, then run sudo riku init again"
        ),
    }
}

fn other_scope(scope: UnitScope) -> UnitScope {
    match scope {
        UnitScope::System => UnitScope::User,
        UnitScope::User => UnitScope::System,
    }
}

/// Both main pids. A failed query is an error, except for the user manager
/// asked from a root init's deploy user child: that child has no user bus
/// (no `XDG_RUNTIME_DIR`), so its user unit counts as not running there.
fn main_pids(scope: UnitScope) -> Result<MainPids> {
    let own = match service_main_pid(scope) {
        Ok(pid) => pid,
        Err(e) => return Err(e),
    };
    let other = match (other_scope(scope), service_main_pid(other_scope(scope))) {
        (_, Ok(pid)) => pid,
        (UnitScope::User, Err(_)) => None,
        (UnitScope::System, Err(e)) => return Err(e),
    };
    Ok(MainPids { own, other })
}

/// Stops a supervisor running outside the `scope` unit and says so on the
/// rail. Prints nothing when there is none. An error when it does not stop
/// within [`STOP_WAIT`], since the unit cannot run while it holds the lock,
/// and when the other scope's `riku` unit holds the lock, since systemd
/// would restart that one and it would fight the unit init installs.
pub(super) fn replace_outside_supervisor(paths: &RikuPaths, scope: UnitScope) -> Result<()> {
    let proc_root = Path::new("/proc");
    let seen = match seen_supervisor(paths, proc_root) {
        Some(seen) => seen,
        None => return Ok(()),
    };
    let units = match main_pids(scope) {
        Ok(units) => units,
        Err(e) => {
            return Err(
                e.context("cannot tell whether systemd runs the supervisor, so it is not stopped")
            )
        }
    };
    let pid = match holder(units, seen.pid) {
        Holder::OwnUnit => return Ok(()),
        Holder::OtherUnit => return Err(other_unit_error(scope)),
        Holder::Outside(pid) => pid,
    };
    if !stop_supervisor(paths, &seen, proc_root, STOP_WAIT) {
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
