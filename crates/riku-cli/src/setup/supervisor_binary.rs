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
use std::process::Command;

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

/// Which systemd manager runs the `riku` unit.
#[derive(Debug, Clone, Copy)]
pub(super) enum UnitScope {
    /// `/etc/systemd/system/riku.service`, written by a root init.
    System,
    /// The plain user's `~/.config/systemd/user/riku.service`.
    User,
}

/// The service's main pid from `systemctl show -p MainPID --value`, which
/// prints `0` for a stopped service.
pub(super) fn parse_main_pid(output: &str) -> Option<u32> {
    match output.trim().parse::<u32>() {
        Ok(0) | Err(_) => None,
        Ok(pid) => Some(pid),
    }
}

/// The main pid of the `riku` unit in `scope`, `None` when it is stopped or
/// systemd cannot be asked.
pub(super) fn service_main_pid(scope: UnitScope) -> Option<u32> {
    let mut command = Command::new("systemctl");
    if let UnitScope::User = scope {
        command.arg("--user");
    }
    command
        .args(["show", "-p", "MainPID", "--value", "riku"])
        .output()
        .ok()
        .and_then(|out| parse_main_pid(&String::from_utf8_lossy(&out.stdout)))
}

/// What to do with a service whose main process is `main_pid`, given the
/// `installed` binary. `proc_root` is `/proc` outside tests.
pub(super) fn action_for(
    main_pid: Option<u32>,
    proc_root: &Path,
    installed: &Path,
) -> ServiceAction {
    match main_pid {
        None => ServiceAction::Start,
        Some(pid) if runs_binary(proc_root, pid, installed) => ServiceAction::KeepRunning,
        Some(_) => ServiceAction::Restart,
    }
}

/// Whether `<proc_root>/<pid>/exe` is the `installed` binary.
pub(super) fn runs_binary(proc_root: &Path, pid: u32, installed: &Path) -> bool {
    same_binary(&proc_root.join(pid.to_string()).join("exe"), installed)
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
