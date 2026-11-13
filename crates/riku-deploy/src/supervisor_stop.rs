//! Stopping the running supervisor, so `riku init` can replace one a deploy
//! started outside systemd with the unit's own.
//!
//! Security: the pid comes from `supervisor.pid` only while a process holds
//! that file's lock (see [`crate::supervisor_ctl::running_supervisor_pid`]).
//!
//! On Linux a pidfd is opened for that pid first. Then the lock is probed
//! again and the pid's start time in `<proc>/<pid>/stat` compared with the
//! one seen at first, and SIGTERM goes through the pidfd. A pidfd names one
//! process for its whole life and never a later one that reuses the pid, so
//! once the check after the open passes, the signal can only reach the
//! process that was checked, or fail if it has exited. A kernel older than
//! 5.3 has no `pidfd_open` (`ENOSYS`); there, and on every other system, the
//! same check runs and is followed by plain `kill`, which leaves a window of
//! microseconds in which a reused pid could be signalled. Without a `/proc`
//! (not Linux) only the lock is checked.

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

fn still_holds_lock(paths: &RikuPaths, seen: &SeenSupervisor, proc_root: &Path) -> bool {
    seen_supervisor(paths, proc_root).as_ref() == Some(seen)
}

/// SIGTERM to `seen` by pid, after checking it still holds the lock.
fn check_then_kill(paths: &RikuPaths, seen: &SeenSupervisor, proc_root: &Path) -> bool {
    still_holds_lock(paths, seen, proc_root)
        && kill(Pid::from_raw(seen.pid), Signal::SIGTERM).is_ok()
}

/// SIGTERM to `seen` through a pidfd opened before the check, so a pid
/// reused after the check is never signalled. True when the signal was sent.
#[cfg(target_os = "linux")]
fn signal_checked(paths: &RikuPaths, seen: &SeenSupervisor, proc_root: &Path) -> bool {
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};

    // SAFETY: pidfd_open takes a pid and flags and returns a new fd or -1.
    let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, seen.pid, 0) };
    if fd < 0 {
        return match nix::errno::Errno::last() {
            nix::errno::Errno::ENOSYS => check_then_kill(paths, seen, proc_root),
            _ => false,
        };
    }
    // SAFETY: the syscall just returned this fd and nothing else owns it.
    let pidfd = unsafe { OwnedFd::from_raw_fd(fd as i32) };
    if !still_holds_lock(paths, seen, proc_root) {
        return false;
    }
    let null_info: *const libc::siginfo_t = std::ptr::null();
    // SAFETY: a valid pidfd, a signal number, no siginfo and no flags.
    let sent = unsafe {
        libc::syscall(
            libc::SYS_pidfd_send_signal,
            pidfd.as_raw_fd(),
            libc::SIGTERM,
            null_info,
            0,
        )
    };
    sent == 0
}

#[cfg(not(target_os = "linux"))]
fn signal_checked(paths: &RikuPaths, seen: &SeenSupervisor, proc_root: &Path) -> bool {
    check_then_kill(paths, seen, proc_root)
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
    if !signal_checked(paths, seen, proc_root) {
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
