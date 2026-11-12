//! `SpawnedProcess`: wrapper around a running child process with metadata.

use anyhow::{anyhow, Result};
use nix::errno::Errno;
use nix::sys::signal::{kill, killpg, Signal};
use nix::sys::wait::{waitpid, WaitPidFlag, WaitStatus};
use nix::unistd::Pid;
use std::path::Path;
use std::process::Child;

use crate::cgroups::WorkerCgroup;
use crate::config::{HealthCheck, WorkerConfig};
use crate::stats::ResourceUsage;

use super::pid_file::WorkerPidFile;

/// Represents a spawned application process with metadata.
pub struct SpawnedProcess {
    pub pid: Pid,
    pub child: Child,
    pub config: WorkerConfig,
    pub restart_count: u32,
    pub last_restart: std::time::Instant,
    pub health_check_config: Option<HealthCheck>,
    pub consecutive_health_failures: u32,
    /// When the last health probe ran; `None` until the first one.
    pub last_health_probe: Option<std::time::Instant>,
    /// Present only when the worker opted into cgroup v2 isolation. Used to
    /// poll for OOM kills, and killed and removed when the worker stops.
    pub cgroup: Option<WorkerCgroup>,
    /// Exit code from the most recent crash, captured by `is_running()` at
    /// the moment `try_wait()` first observes the process has died, used to
    /// annotate the `app.restarted` event fired for that crash.
    pub last_exit_code: Option<i32>,
    /// Marker telling deploy this process is alive. Removed in `Drop` once
    /// the child is reaped, never earlier.
    pid_file: Option<WorkerPidFile>,
    /// Set by `stop`, so `Drop` only stops a process nobody stopped.
    pub(super) stopped: bool,
}

impl SpawnedProcess {
    /// Create a new SpawnedProcess instance, attaching the cgroup that was
    /// provisioned for it (if isolation is enabled for this worker) and
    /// writing its pid marker into `pid_dir` when one is given.
    #[allow(clippy::question_mark)]
    pub fn new_with_cgroup(
        child: Child,
        config: WorkerConfig,
        cgroup: Option<WorkerCgroup>,
        pid_dir: Option<&Path>,
    ) -> Result<Self> {
        let pid = Pid::from_raw(child.id() as i32);
        let health_check_config = config.options.health_check.clone();
        let pid_file = match pid_dir {
            Some(dir) => {
                let process_id = config.worker.name().to_string();
                match WorkerPidFile::create(dir, &process_id, child.id()) {
                    Ok(marker) => Some(marker),
                    Err(e) => return Err(e),
                }
            }
            None => None,
        };

        Ok(SpawnedProcess {
            pid,
            child,
            config,
            restart_count: 0,
            last_restart: std::time::Instant::now(),
            health_check_config,
            consecutive_health_failures: 0,
            last_health_probe: None,
            cgroup,
            last_exit_code: None,
            pid_file,
            stopped: false,
        })
    }

    /// Cumulative OOM-kill count reported by the worker's cgroup, or `None`
    /// if isolation isn't enabled for this worker.
    pub fn oom_kill_count(&self) -> Option<u64> {
        self.cgroup.as_ref().and_then(|c| c.oom_kill_count())
    }

    /// CPU time and memory of the whole worker: its cgroup when it has one,
    /// otherwise its process tree in `procs`.
    pub fn resource_usage(&self, procs: impl FnOnce() -> ResourceUsage) -> ResourceUsage {
        match self.cgroup.as_ref().and_then(WorkerCgroup::usage) {
            Some(usage) => usage,
            None => procs(),
        }
    }

    /// The memory limit the worker runs under: its cgroup `memory.max`,
    /// else the limit its config sets.
    pub fn memory_limit(&self) -> Option<u64> {
        let configured = self
            .config
            .options
            .limits
            .as_ref()
            .and_then(|limits| limits.max_memory_bytes);
        match &self.cgroup {
            Some(cgroup) => cgroup.memory_max().or(configured),
            None => configured,
        }
    }

    /// Check if the process is still running.
    pub fn is_running(&mut self) -> bool {
        match self.child.try_wait() {
            Ok(Some(status)) => {
                self.last_exit_code = status.code();
                false // Process has exited
            }
            Ok(None) => true, // Process is still running
            Err(_) => false,  // Error checking status, assume dead
        }
    }

    /// SIGTERM the worker's whole process group.
    pub fn terminate(&mut self) -> Result<()> {
        self.signal_group(Signal::SIGTERM)
    }

    /// SIGKILL the worker's whole process group.
    pub fn kill(&mut self) -> Result<()> {
        self.signal_group(Signal::SIGKILL)
    }

    /// The group id is the leader's pid, since workers spawn with
    /// `process_group(0)`. An empty group is not an error: a crashed
    /// worker may have nothing left to signal.
    fn signal_group(&self, signal: Signal) -> Result<()> {
        match killpg(self.pid, signal) {
            Ok(()) | Err(Errno::ESRCH) => Ok(()),
            Err(e) => Err(anyhow!("cannot send {signal} to group {}: {e}", self.pid)),
        }
    }

    /// Whether any process is left in the worker's process group. Call only
    /// once the leader has been reaped: this reaps any child of ours in the
    /// group, which would include the leader.
    pub fn group_alive(&self) -> bool {
        self.reap_group_zombies();
        match kill(Pid::from_raw(-self.pid.as_raw()), None) {
            Ok(()) => true,
            Err(Errno::EPERM) => true,
            Err(_) => false,
        }
    }

    /// Group members whose parent died are reparented to PID 1. When the
    /// supervisor is PID 1 (in a container) they become its children, and
    /// once they exit they stay zombies in the group until reaped. Anywhere
    /// else the members are not our children and `waitpid` finds nothing.
    fn reap_group_zombies(&self) {
        let group = Pid::from_raw(-self.pid.as_raw());
        while let Ok(status) = waitpid(group, Some(WaitPidFlag::WNOHANG)) {
            if status == WaitStatus::StillAlive {
                break;
            }
        }
    }

    /// Get the process ID as u32.
    pub fn pid_as_u32(&self) -> u32 {
        self.pid.as_raw() as u32
    }
}

impl Drop for SpawnedProcess {
    /// A crashed worker, or one dropped without a stop, still gets the full
    /// stop: its group leftovers and its cgroup would otherwise outlive it.
    fn drop(&mut self) {
        if !self.stopped {
            let process_id = self.config.worker.name().to_string();
            if let Err(e) = self.stop(&process_id) {
                tracing::warn!("cleanup of {process_id} failed: {e:#}");
            }
        }
        drop(self.pid_file.take());
    }
}

#[cfg(test)]
#[path = "spawned_tests.rs"]
mod tests;
