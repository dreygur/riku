//! cgroups v2 resource constraints for spawned worker processes.
//!
//! Provisions one cgroup per worker under `/sys/fs/cgroup/riku/apps/<id>/`
//! and writes `memory.max` / `cpu.max` to the unified (v2) control group
//! filesystem before the worker starts running, so it is constrained from
//! its very first instruction. Requires cgroup v2 to be mounted at
//! `/sys/fs/cgroup` and write access to it (root, or a delegated subtree).

use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

#[cfg(test)]
mod tests;

const CGROUP_ROOT: &str = "/sys/fs/cgroup/riku/apps";

/// CPU/memory constraints for a single worker's cgroup.
#[derive(Debug, Clone, Default)]
pub struct CgroupLimits {
    /// Hard memory ceiling in bytes, written to `memory.max`. `None` leaves
    /// the kernel default (`max`, i.e. unlimited).
    pub memory_max_bytes: Option<u64>,
    /// CPU quota in microseconds per `cpu_period_us`, written to `cpu.max`
    /// as `"<quota> <period>"`. `None` writes `"max <period>"` (unlimited).
    pub cpu_quota_us: Option<u64>,
    /// CPU accounting period in microseconds (kernel default and most
    /// common choice is 100000, i.e. 100ms).
    pub cpu_period_us: u64,
    /// Maximum number of tasks (processes/threads) this worker's cgroup may
    /// ever contain at once, written to `pids.max`. `None` leaves the
    /// kernel default (`max`, i.e. unlimited).
    ///
    /// This is the correct way to bound a worker's fork count: unlike
    /// `RLIMIT_NPROC` (see `resource_limits::ResourceLimits`), which counts
    /// every process owned by the real UID system-wide and so falsely
    /// trips on an otherwise-healthy worker whenever anything else under
    /// that UID is also running, `pids.max` is scoped to exactly this
    /// cgroup's subtree.
    pub pids_max: Option<u64>,
}

impl CgroupLimits {
    /// Render the `cpu.max` file contents for these limits.
    fn cpu_max_value(&self) -> String {
        match self.cpu_quota_us {
            Some(quota) => format!("{} {}", quota, self.cpu_period_us),
            None => format!("max {}", self.cpu_period_us),
        }
    }
}

/// A provisioned cgroup for one worker process. Cheap to clone (just the
/// path) so a handle can be moved into a `pre_exec` closure while another
/// stays with the supervisor for later polling/cleanup.
#[derive(Clone)]
pub struct WorkerCgroup {
    path: PathBuf,
}

/// Verify the cgroup v2 root riku provisions worker cgroups under actually
/// exists and is writable, by creating and removing a throwaway probe
/// directory under it — the same `create_dir_all` + write pattern
/// `provision` uses for real workers.
///
/// Call this once at supervisor startup so an unmounted cgroup v2
/// filesystem, or a riku user without write access to its delegated
/// subtree, surfaces as one clear warning at boot instead of a deploy-time
/// error deep inside `spawn_process` the first time someone enables
/// isolation on a worker.
pub fn verify_root_writable() -> Result<()> {
    let probe = Path::new(CGROUP_ROOT).join(".riku-startup-probe");
    fs::create_dir_all(&probe)
        .with_context(|| format!("cgroup v2 root not writable: {}", probe.display()))?;
    fs::remove_dir(&probe).ok();
    Ok(())
}

/// Render a structured, human-readable startup diagnostic for a
/// `verify_root_writable` failure: what failed, why it matters, and how to
/// fix it. Used instead of a one-line warning so operators running in
/// production don't have to go spelunking in `spawn_process` source to
/// understand the failure the first time a worker requests isolation.
pub fn startup_diagnostic(err: &anyhow::Error) -> String {
    format!(
        "riku startup diagnostic\n\
         \x20 component : cgroup v2 resource isolation\n\
         \x20 check     : write access to {root}\n\
         \x20 status    : FAILED\n\
         \x20 detail    : {err}\n\
         \x20 impact    : any worker with `isolation` enabled in its config will fail to\n\
         \x20             start the moment it's deployed — the failure surfaces deep in\n\
         \x20             spawn_process, not here, unless this check catches it first\n\
         \x20 remedy    : mount cgroup v2 at /sys/fs/cgroup (most modern distros do this by\n\
         \x20             default) and ensure the riku user has write access to {root},\n\
         \x20             e.g. by delegating that subtree to it (systemd cgroup delegation\n\
         \x20             or `chown -R` if running riku as a dedicated user)",
        root = CGROUP_ROOT,
        err = err,
    )
}

impl WorkerCgroup {
    /// Create (or reuse) the cgroup directory for `process_id` under the
    /// riku cgroup root and write `limits` to it. Call this from the
    /// supervisor (parent) before spawning the worker, so the constraints
    /// already exist by the time the worker joins via `add_self`.
    pub fn provision(process_id: &str, limits: &CgroupLimits) -> Result<Self> {
        let path = Path::new(CGROUP_ROOT).join(process_id);
        fs::create_dir_all(&path)
            .with_context(|| format!("creating cgroup directory {}", path.display()))?;

        if let Some(bytes) = limits.memory_max_bytes {
            write_node(&path, "memory.max", &bytes.to_string())?;
        }
        write_node(&path, "cpu.max", &limits.cpu_max_value())?;
        if let Some(pids) = limits.pids_max {
            write_node(&path, "pids.max", &pids.to_string())?;
        }

        Ok(WorkerCgroup { path })
    }

    /// Move the *calling* process into this cgroup by writing its own PID
    /// to `cgroup.procs`. Must be called from within the worker's own
    /// `pre_exec` hook (after fork, before exec) — joining from there,
    /// using the worker's real top-level PID, avoids the race that would
    /// exist if the parent tried to add the PID after `Command::spawn()`
    /// returns (by then the worker may already have started running
    /// unconstrained).
    pub fn add_self(&self) -> std::io::Result<()> {
        let pid = nix::unistd::getpid();
        std::fs::write(self.path.join("cgroup.procs"), pid.to_string())
    }

    /// Read `memory.events` and return the cumulative `oom_kill` counter,
    /// or `None` if the node can't be read (e.g. cgroup already removed).
    pub fn oom_kill_count(&self) -> Option<u64> {
        let content = fs::read_to_string(self.path.join("memory.events")).ok()?;
        content
            .lines()
            .find_map(|line| line.strip_prefix("oom_kill "))
            .and_then(|value| value.trim().parse().ok())
    }

    /// Remove the cgroup directory. Must be called after the worker has
    /// exited — the kernel refuses to rmdir a non-empty cgroup.
    pub fn cleanup(&self) -> Result<()> {
        match fs::remove_dir(&self.path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e).context(format!("removing cgroup {}", self.path.display())),
        }
    }
}

fn write_node(dir: &Path, node: &str, value: &str) -> Result<()> {
    fs::write(dir.join(node), value)
        .with_context(|| format!("writing {} to {}/{}", value, dir.display(), node))
}
