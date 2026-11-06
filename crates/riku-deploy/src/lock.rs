//! Per-app deploy lock.
//!
//! `do_deploy` mutates state shared across concurrent triggers for the same
//! app: it `git reset --hard`s the working tree, rewrites worker TOML
//! configs, and read-modify-writes the ENV file's routing keys.
//! Two deploys for the same app racing (a second `git push` landing while
//! the first `post-receive` hook is still running, or a dashboard-triggered
//! redeploy firing mid-push) corrupt that shared state, e.g. a lost update
//! on the routing keys in ENV leaves nginx on the wrong template. An
//! advisory `flock` keyed by app name serializes deploys of the same app
//! without affecting deploys of different apps.
//!
//! The holder writes what it is into the lock file. A deploy that finds the
//! lock taken by the supervisor restarting a crashed container waits for the
//! restart to end instead of being refused, since a restart is bounded and a
//! push should not fail because a copy happened to crash just before it.

use anyhow::{Context, Result};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::io::{AsRawFd, RawFd};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use crate::config::RikuPaths;
use crate::error::DeployError;
use crate::rolling::health::HEALTHCHECK_WAIT;

const CONTENTION_GRACE: Duration = Duration::from_secs(1);
const RETRY_INTERVAL: Duration = Duration::from_millis(10);

/// How long a deploy waits out a supervisor restart: the longest health wait
/// a restart can sit in, plus time to replace the container and reconfigure
/// the router.
const RESTART_WAIT: Duration = Duration::from_secs(HEALTHCHECK_WAIT.as_secs() + 30);

/// What holds an app's lock, as recorded in the lock file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Holder {
    /// A deploy or a user-started operation on the app.
    Operation,
    /// The supervisor replacing one crashed container copy.
    Restart,
}

impl Holder {
    fn tag(self) -> &'static str {
        match self {
            Holder::Operation => "operation",
            Holder::Restart => "restart",
        }
    }
}

/// Apply a non-blocking `flock` operation, retrying on `EINTR`.
///
/// `flock(2)` is interruptible: a signal delivered to the process (e.g.
/// `SIGCHLD` when an unrelated child process exits) can interrupt the syscall
/// and make it return `EINTR`. Treating that as "contended" would spuriously
/// report a lock as held: and, in `acquire`, fail a legitimate deploy, so we
/// retry on `EINTR` and only report `false` for a genuine would-block.
///
/// Returns `Ok(true)` if the operation acquired/changed the lock, `Ok(false)`
/// if it would block (another holder), or `Err` for any other failure.
fn try_flock(fd: RawFd, operation: libc::c_int) -> io::Result<bool> {
    loop {
        let rc = unsafe { libc::flock(fd, operation) };
        if rc == 0 {
            return Ok(true);
        }
        let err = io::Error::last_os_error();
        match err.raw_os_error() {
            Some(libc::EINTR) => continue,
            // EAGAIN == EWOULDBLOCK on Linux; this is "another process holds it".
            Some(libc::EWOULDBLOCK) => return Ok(false),
            _ => return Err(err),
        }
    }
}

fn held_by_restart(lock_path: &Path) -> bool {
    match fs::read_to_string(lock_path) {
        Ok(tag) => tag.trim() == Holder::Restart.tag(),
        Err(_) => false,
    }
}

/// Whether a contender that has waited `waited` keeps polling.
///
/// A child process spawned by a concurrent thread can transiently inherit
/// the lock file descriptor across fork until exec closes it. A real deploy
/// holds the lock far longer than 1 s, so every contender waits out any
/// transient child inheritance before declaring contention. Past that, only
/// a contender allowed to outlast a restart keeps waiting, and only while
/// the holder is a restart.
fn keep_waiting(waited: Duration, restart_wait: Duration, lock_path: &Path) -> bool {
    if waited < CONTENTION_GRACE {
        return true;
    }
    waited < restart_wait && held_by_restart(lock_path)
}

fn retry_acquire(file: &File, lock_path: &Path, restart_wait: Duration) -> io::Result<bool> {
    let start = Instant::now();
    loop {
        thread::sleep(RETRY_INTERVAL);
        match try_flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) {
            Ok(true) => return Ok(true),
            Ok(false) => {
                if !keep_waiting(start.elapsed(), restart_wait, lock_path) {
                    return Ok(false);
                }
            }
            Err(e) => return Err(e),
        }
    }
}

fn record_holder(mut file: &File, holder: Holder) -> io::Result<()> {
    match file.set_len(0) {
        Ok(()) => file.write_all(holder.tag().as_bytes()),
        Err(e) => Err(e),
    }
}

/// Acquire the deploy lock for `app`, non-blocking. Returns the locked file
/// handle: the lock is held until it is dropped, so callers must keep the
/// returned `File` alive for the duration of the deploy.
///
/// Returns `Err(DeployError::DeployInProgress)` if another deploy for this
/// app already holds the lock.
pub fn acquire(app: &str, paths: &RikuPaths) -> Result<File> {
    acquire_as(app, paths, Holder::Operation, Duration::ZERO)
}

/// [`acquire`] for a deploy. A lock held by a supervisor restart is waited
/// for, up to the longest a restart can take, before the deploy is refused.
pub fn acquire_for_deploy(app: &str, paths: &RikuPaths) -> Result<File> {
    acquire_as(app, paths, Holder::Operation, RESTART_WAIT)
}

/// [`acquire`] for the supervisor restarting a crashed copy. A deploy that
/// finds the lock taken this way waits for it instead of failing.
pub fn acquire_for_restart(app: &str, paths: &RikuPaths) -> Result<File> {
    acquire_as(app, paths, Holder::Restart, Duration::ZERO)
}

fn acquire_as(
    app: &str,
    paths: &RikuPaths,
    holder: Holder,
    restart_wait: Duration,
) -> Result<File> {
    let lock_dir = paths.riku_root.join("locks");
    match fs::create_dir_all(&lock_dir) {
        Ok(()) => {}
        Err(e) => return Err(e).context("failed to create lock directory"),
    }
    let lock_path = lock_path_for(app, paths);

    let file = match OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lock_path)
    {
        Ok(file) => file,
        Err(e) => return Err(e).context("failed to open lock file"),
    };

    // Use libc::flock directly, same as the supervisor's PID-file lock
    // (create_pid_file_with_lock): portable across Unix systems, no extra
    // dependency. EINTR is retried inside try_flock so a stray signal can't
    // make a legitimate deploy look like a concurrent one.
    let acquired = match try_flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) {
        Ok(true) => Ok(true),
        Ok(false) => retry_acquire(&file, &lock_path, restart_wait),
        Err(e) => Err(e),
    };
    match acquired {
        Ok(true) => {}
        Ok(false) => return Err(DeployError::DeployInProgress(app.to_string()).into()),
        Err(e) => return Err(e).context("failed to acquire deploy lock"),
    }
    match record_holder(&file, holder) {
        Ok(()) => Ok(file),
        Err(e) => Err(e).context("failed to record the deploy lock holder"),
    }
}

fn lock_path_for(app: &str, paths: &RikuPaths) -> PathBuf {
    paths
        .riku_root
        .join("locks")
        .join(format!("{}.deploy.lock", app))
}

/// Read-only probe: is `app`'s deploy lock currently held by another
/// process? Used by `riku __dump-state` to report live lock state without
/// ever taking the lock itself.
///
/// Implemented as a non-blocking `flock` attempt that's immediately
/// released on success: the only race-free way to ask "is this locked"
/// without disturbing a genuine holder: open a *fresh* fd (never the
/// holder's), try `LOCK_EX | LOCK_NB`. Success means nobody held it (and
/// this probe's own momentary lock is dropped immediately after); `EWOULDBLOCK`
/// means somebody does.
///
/// Returns `false` (not held) if the lock file doesn't exist yet, or if it
/// can't be opened at all: under-reporting "free" is the safe default for
/// a monitoring dump, since this is informational only and never gates a
/// real deploy decision.
pub fn is_locked(app: &str, paths: &RikuPaths) -> bool {
    let lock_path = lock_path_for(app, paths);
    let file = match OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lock_path)
    {
        Ok(f) => f,
        Err(_) => return false,
    };

    match try_flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) {
        Ok(true) => {
            // We got it: nobody else holds it. Release immediately; dropping
            // `file` closes the fd, which releases the flock too, but we
            // unlock explicitly first so there's no window where this probe
            // itself looks like a held lock to a concurrent probe.
            let _ = try_flock(file.as_raw_fd(), libc::LOCK_UN);
            false
        }
        // Genuinely contended → held by someone else.
        Ok(false) => true,
        // Can't determine (unexpected error). Under-report "free", this is a
        // best-effort monitoring probe that never gates a real deploy.
        Err(_) => false,
    }
}

#[cfg(test)]
#[path = "lock_tests.rs"]
mod tests;
