//! What `riku init` does to the `riku` service it just wrote: start it when
//! it is down, leave it running when it already runs the installed binary,
//! restart it only when that binary changed. Every restart stops every app's
//! workers, so re-running init without an upgrade must not cause one.
//!
//! "Changed" compares the running process's binary, read through
//! `/proc/<pid>/exe`, with the installed one. That link still opens the old
//! file after the install renamed a new one over it, so the comparison holds
//! on an upgrade: same inode, or same bytes, means unchanged. An exe that
//! cannot be read counts as changed, so an upgrade is never left half done.

use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

/// The `systemctl` action init takes on the service.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum ServiceAction {
    Start,
    KeepRunning,
    Restart,
}

impl ServiceAction {
    /// `start` is a no-op for a running service, so it also keeps one.
    pub(super) fn verb(&self) -> &'static str {
        match self {
            ServiceAction::Start | ServiceAction::KeepRunning => "start",
            ServiceAction::Restart => "restart",
        }
    }

    /// The word on the `Service` step when the verb succeeded.
    pub(super) fn outcome(&self) -> &'static str {
        match self {
            ServiceAction::Start => "started",
            ServiceAction::KeepRunning => "running",
            ServiceAction::Restart => "restarted (new binary)",
        }
    }
}

/// The service's main pid from `systemctl show -p MainPID --value`, which
/// prints `0` for a stopped service.
pub(super) fn parse_main_pid(output: &str) -> Option<u32> {
    match output.trim().parse::<u32>() {
        Ok(0) | Err(_) => None,
        Ok(pid) => Some(pid),
    }
}

/// What to do with a service whose main process is `main_pid`, given the
/// `installed` binary. `proc_root` is `/proc` outside tests.
pub(super) fn action_for(
    main_pid: Option<u32>,
    proc_root: &Path,
    installed: &Path,
) -> ServiceAction {
    let pid = match main_pid {
        Some(pid) => pid,
        None => return ServiceAction::Start,
    };
    let running = proc_root.join(pid.to_string()).join("exe");
    if same_binary(&running, installed) {
        ServiceAction::KeepRunning
    } else {
        ServiceAction::Restart
    }
}

fn same_binary(running: &Path, installed: &Path) -> bool {
    let (running_meta, installed_meta) = match (fs::metadata(running), fs::metadata(installed)) {
        (Ok(running), Ok(installed)) => (running, installed),
        _ => return false,
    };
    if running_meta.dev() == installed_meta.dev() && running_meta.ino() == installed_meta.ino() {
        return true;
    }
    if running_meta.len() != installed_meta.len() {
        return false;
    }
    match (fs::read(running), fs::read(installed)) {
        (Ok(running), Ok(installed)) => running == installed,
        _ => false,
    }
}

#[cfg(test)]
#[path = "supervisor_binary_tests.rs"]
mod tests;
