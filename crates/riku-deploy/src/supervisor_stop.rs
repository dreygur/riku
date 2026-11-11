//! Stopping the running supervisor, so `riku init` can replace one a deploy
//! started outside systemd with the unit's own.
//!
//! Security: the pid comes from `supervisor.pid` only while a process holds
//! that file's lock (see [`crate::supervisor_ctl::running_supervisor_pid`]),
//! so a pid since reused by an unrelated process is never signalled.

use std::thread;
use std::time::{Duration, Instant};

use nix::sys::signal::{kill, Signal};
use nix::unistd::Pid;

use crate::config::RikuPaths;
use crate::supervisor_ctl::pid_file_locked;

const POLL: Duration = Duration::from_millis(100);

/// Sends SIGTERM to the supervisor `pid` and waits up to `wait` for its pid
/// file lock to free, which happens once it has stopped its workers and
/// exited. True when the lock is free.
pub fn stop_supervisor(paths: &RikuPaths, pid: i32, wait: Duration) -> bool {
    if kill(Pid::from_raw(pid), Signal::SIGTERM).is_err() {
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
