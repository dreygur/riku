//! Stopping the running supervisor, so `riku init` can replace one a deploy
//! started outside systemd with the unit's own.
//!
//! Security: the pid comes from `supervisor.pid` only while a process holds
//! that file's lock (see [`crate::supervisor_ctl::running_supervisor_pid`]).
//! Right before the signal the lock is probed again and the pid's start
//! time in `<proc>/<pid>/stat` compared with the one seen at first, so a pid
//! that was reused since is not signalled. The window between that check
//! and `kill` remains; it is microseconds, not the seconds init spends
//! between finding the supervisor and stopping it. Without a `/proc` (not
//! Linux) only the lock is checked.

// Project rule 16 forbids `?`, and this lint asks for exactly that rewrite.
#![allow(clippy::question_mark)]

use std::fs;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use nix::sys::signal::{kill, Signal};
use nix::unistd::Pid;

use crate::config::RikuPaths;
use crate::supervisor_ctl::{pid_file_locked, running_supervisor_pid};

const POLL: Duration = Duration::from_millis(100);

/// `starttime`, the 22nd field of `/proc/<pid>/stat`, counted after the
/// `(comm)` field, which may itself hold spaces or parentheses.
const START_TIME_AFTER_COMM: usize = 19;

/// A supervisor seen holding the pid file lock.
#[derive(Debug, PartialEq, Eq)]
pub struct SeenSupervisor {
    /// Its pid, always positive.
    pub pid: i32,
    /// When it started, from `<proc_root>/<pid>/stat`; `None` without one.
    start_time: Option<u64>,
}

fn start_time(proc_root: &Path, pid: i32) -> Option<u64> {
    let stat = match fs::read_to_string(proc_root.join(pid.to_string()).join("stat")) {
        Ok(stat) => stat,
        Err(_) => return None,
    };
    let after_comm = match stat.rfind(')') {
        Some(end) => &stat[end + 1..],
        None => return None,
    };
    after_comm
        .split_whitespace()
        .nth(START_TIME_AFTER_COMM)
        .and_then(|field| field.parse().ok())
}

/// The supervisor holding the lock now, with its start time.
pub fn seen_supervisor(paths: &RikuPaths, proc_root: &Path) -> Option<SeenSupervisor> {
    running_supervisor_pid(paths).map(|pid| SeenSupervisor {
        pid,
        start_time: start_time(proc_root, pid),
    })
}

/// Sends SIGTERM to the supervisor `seen` and waits up to `wait` for its
/// pid file lock to free, which happens once it has stopped its workers and
/// exited. True when the lock is free. Nothing is signalled when the lock is
/// no longer held by that same process.
pub fn stop_supervisor(
    paths: &RikuPaths,
    seen: &SeenSupervisor,
    proc_root: &Path,
    wait: Duration,
) -> bool {
    if seen_supervisor(paths, proc_root).as_ref() != Some(seen) {
        return !pid_file_locked(paths);
    }
    if kill(Pid::from_raw(seen.pid), Signal::SIGTERM).is_err() {
        return !pid_file_locked(paths);
    }
    let deadline = Instant::now() + wait;
    while pid_file_locked(paths) {
        if Instant::now() >= deadline {
            return false;
        }
        thread::sleep(POLL);
    }
    true
}

#[cfg(test)]
#[path = "supervisor_stop_tests.rs"]
mod tests;
