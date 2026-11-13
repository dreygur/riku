//! Reading `supervisor.pid`, the file a running supervisor keeps locked.
//!
//! Security: the file lives in the deploy user's tree, and root reaches this
//! code too (`riku doctor`, `riku init`). It is opened once with
//! `O_NOFOLLOW | O_NONBLOCK` and used only when that handle is a regular file
//! owned by the effective user, so a planted link, a FIFO or another user's
//! file reads as no pid file and never blocks. The lock is probed on the same
//! handle and at most [`MAX_LEN`] bytes are read from it.
//!
//! Root still opens the deploy user's file this way, but only its metadata
//! is read: the owner check fails, so neither the lock nor the pid is used
//! and root sees no pid file. A root caller that needs the answer says it
//! skipped the check (`riku doctor`'s supervisor binary line).

// Project rule 16 forbids `?`, and this lint asks for exactly that rewrite.
#![allow(clippy::question_mark)]

use std::fs::File;
use std::io::Read;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::Path;

/// The pid file's name under the riku root.
pub const PID_FILE: &str = "supervisor.pid";

/// Longer than any pid plus a newline; the rest of a bigger file is ignored.
const MAX_LEN: u64 = 32;

/// What the pid file held when it was probed.
#[derive(Debug, PartialEq, Eq)]
pub struct PidFile {
    /// The positive pid it names; `None` for anything else, since `0` or a
    /// negative pid would signal a whole process group.
    pub pid: Option<i32>,
    /// Some process holds the file's `flock`, which a supervisor keeps for its
    /// whole life. An unlocked file is left over from a dead one.
    pub locked: bool,
}

fn open_owned(path: &Path) -> Option<File> {
    let file = File::options()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
        .open(path);
    let file = match file {
        Ok(file) => file,
        Err(_) => return None,
    };
    match file.metadata() {
        Ok(meta) if meta.is_file() && meta.uid() == nix::unistd::geteuid().as_raw() => Some(file),
        _ => None,
    }
}

fn parse_pid(text: &str) -> Option<i32> {
    match text.trim().parse::<i32>() {
        Ok(pid) if pid > 0 => Some(pid),
        _ => None,
    }
}

/// The pid file at `path`, or `None` when it is absent or refused (see the
/// module doc). The lock probe takes a shared lock for an instant, so two
/// probes never block each other.
pub fn probe(path: &Path) -> Option<PidFile> {
    let file = match open_owned(path) {
        Some(file) => file,
        None => return None,
    };
    let locked = matches!(
        file.try_lock_shared(),
        Err(std::fs::TryLockError::WouldBlock)
    );
    let mut text = String::new();
    let pid = match (&file).take(MAX_LEN).read_to_string(&mut text) {
        Ok(_) => parse_pid(&text),
        Err(_) => None,
    };
    Some(PidFile { pid, locked })
}

/// The pid of the supervisor holding the lock on the pid file at `path`.
pub fn running_pid(path: &Path) -> Option<i32> {
    match probe(path) {
        Some(PidFile {
            pid: Some(pid),
            locked: true,
        }) => Some(pid),
        _ => None,
    }
}

#[cfg(test)]
#[path = "supervisor_pid_tests.rs"]
mod tests;
