//! `SpawnedProcess` — wrapper around a running child process with metadata.

use anyhow::Result;
use nix::sys::signal::Signal;
use nix::unistd::Pid;
use std::fs::File;
use std::process::Child;
use std::thread;
use std::time::Duration;

use crate::supervisor::cgroups::WorkerCgroup;
use crate::supervisor::config::{HealthCheck, WorkerConfig};

/// Represents a spawned application process with metadata.
pub struct SpawnedProcess {
    pub pid: Pid,
    pub child: Child,
    pub config: WorkerConfig,
    pub restart_count: u32,
    pub last_restart: std::time::Instant,
    pub health_check_config: Option<HealthCheck>,
    pub consecutive_health_failures: u32,
    /// Present only when the worker opted into cgroup v2 isolation. Used to
    /// poll for OOM kills and removed once the process has exited.
    pub cgroup: Option<WorkerCgroup>,
    #[allow(dead_code)]
    log_handles: Option<(File, File)>,
}

impl SpawnedProcess {
    /// Create a new SpawnedProcess instance, attaching the cgroup that was
    /// provisioned for it (if isolation is enabled for this worker).
    pub fn new_with_cgroup(
        child: Child,
        config: WorkerConfig,
        log_handles: Option<(File, File)>,
        cgroup: Option<WorkerCgroup>,
    ) -> Result<Self> {
        let pid = Pid::from_raw(child.id() as i32);
        let health_check_config = config.options.health_check.clone();

        Ok(SpawnedProcess {
            pid,
            child,
            config,
            restart_count: 0,
            last_restart: std::time::Instant::now(),
            health_check_config,
            consecutive_health_failures: 0,
            cgroup,
            log_handles,
        })
    }

    /// Cumulative OOM-kill count reported by the worker's cgroup, or `None`
    /// if isolation isn't enabled for this worker.
    pub fn oom_kill_count(&self) -> Option<u64> {
        self.cgroup.as_ref().and_then(|c| c.oom_kill_count())
    }

    /// Check if the process is still running.
    pub fn is_running(&mut self) -> bool {
        match self.child.try_wait() {
            Ok(Some(_status)) => false, // Process has exited
            Ok(None) => true,           // Process is still running
            Err(_) => false,            // Error checking status, assume dead
        }
    }

    /// Send a termination signal to the process and its entire process group.
    ///
    /// This kills all child processes spawned by the main process,
    /// preventing orphaned background jobs.
    pub fn terminate(&mut self) -> Result<()> {
        use nix::sys::signal::killpg;

        // Kill the entire process group (PGID == PID since we used process_group(0))
        killpg(self.pid, Signal::SIGTERM)?;
        Ok(())
    }

    /// Force kill the process and its entire process group.
    pub fn kill(&mut self) -> Result<()> {
        use nix::sys::signal::killpg;

        // Force kill the entire process group
        killpg(self.pid, Signal::SIGKILL)?;
        Ok(())
    }

    /// Get the process ID as u32.
    pub fn pid_as_u32(&self) -> u32 {
        self.pid.as_raw() as u32
    }
}

impl Drop for SpawnedProcess {
    fn drop(&mut self) {
        // Ensure the child process is cleaned up when SpawnedProcess is dropped.
        // Child::drop does NOT kill the process, so we must do it explicitly.
        if self.is_running() {
            let _ = self.terminate();
            // Give brief time for graceful shutdown
            thread::sleep(Duration::from_millis(100));
            if self.is_running() {
                let _ = self.kill();
            }
        }
        // Reap the child to avoid zombies
        let _ = self.child.wait();

        // The cgroup must be empty before it can be removed, which is only
        // guaranteed once the child (and any namespace-isolation shim) has
        // been reaped above.
        if let Some(cgroup) = &self.cgroup {
            let _ = cgroup.cleanup();
        }
    }
}
