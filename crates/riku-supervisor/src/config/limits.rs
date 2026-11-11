//! The kernel constraints a worker can ask for, kept as two separate
//! settings: cgroup resource limits, and the full sandbox (chroot plus
//! PID, mount and network namespaces). A worker can have limits without a
//! sandbox. A sandboxed worker always runs in its own cgroup, with or
//! without limits.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// cgroup v2 limits for one worker, from the `RIKU_ISOLATION_*` limit
/// settings. Present only when at least one real limit is set.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct WorkerLimits {
    /// Hard memory ceiling in bytes (`memory.max`).
    #[serde(default)]
    pub max_memory_bytes: Option<u64>,
    /// CPU quota in microseconds per `cpu_period_us` (`cpu.max`). `None`
    /// leaves CPU unconstrained.
    #[serde(default)]
    pub cpu_quota_us: Option<u64>,
    /// CPU accounting period in microseconds. Only used with a quota.
    #[serde(default = "default_cpu_period_us")]
    pub cpu_period_us: u64,
    /// Maximum number of tasks (processes and threads) in the worker's
    /// cgroup at once (`pids.max`). Unlike `RLIMIT_NPROC`, which counts
    /// every process of the UID system wide, this counts only the worker's
    /// own subtree.
    #[serde(default)]
    pub max_pids: Option<u64>,
}

fn default_cpu_period_us() -> u64 {
    100_000
}

impl WorkerLimits {
    /// The limits `env` sets, or `None` when it sets no memory, CPU quota
    /// or pids limit. A CPU period alone limits nothing, so it asks for no
    /// cgroup.
    pub(super) fn from_env(env: &HashMap<String, String>) -> Option<Self> {
        let limits = WorkerLimits {
            max_memory_bytes: parse_env_opt!(env, "RIKU_ISOLATION_MAX_MEMORY_MB", u64)
                .map(|mb| mb * 1024 * 1024),
            cpu_quota_us: parse_env_opt!(env, "RIKU_ISOLATION_CPU_QUOTA_US", u64),
            cpu_period_us: parse_env_or!(
                env,
                "RIKU_ISOLATION_CPU_PERIOD_US",
                u64,
                default_cpu_period_us
            ),
            max_pids: parse_env_opt!(env, "RIKU_ISOLATION_MAX_PIDS", u64),
        };
        limits.limits_something().then_some(limits)
    }

    /// Whether a memory, CPU quota or pids limit is set.
    pub(super) fn limits_something(&self) -> bool {
        self.max_memory_bytes.is_some() || self.cpu_quota_us.is_some() || self.max_pids.is_some()
    }
}

/// The full sandbox, opt in through `RIKU_ISOLATION_ROOT`. The worker is
/// started through `riku __ns-shim`, which `pivot_root`s it into
/// `root_dir` and unshares PID, mount and network namespaces. The network
/// namespace leaves the worker only loopback, so it cannot reach the
/// router, a database or anything else on the host network.
///
/// Serialized as `[options.isolation]`, the key older configs used for
/// both the sandbox and the limits. `options_file` reads it, along with any
/// limits an older config kept beside `root_dir`.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Sandbox {
    /// Directory to `pivot_root` the worker into. Must hold everything the
    /// worker needs (app code, libraries, `/proc`, `/dev`).
    pub root_dir: String,
}

impl Sandbox {
    pub(super) fn from_env(env: &HashMap<String, String>) -> Option<Self> {
        env.get("RIKU_ISOLATION_ROOT").map(|root_dir| Sandbox {
            root_dir: root_dir.clone(),
        })
    }

    /// Refuses a `root_dir` without the shape `riku settings set` requires
    /// (riku-config `is_isolation_root`) or that is not an existing
    /// directory. A worker config can also come from an older ENV file or be
    /// edited by hand, and the directory can vanish after it was set.
    pub fn check_root(&self) -> anyhow::Result<()> {
        if !riku_config::app_settings::is_isolation_root(&self.root_dir) {
            anyhow::bail!(
                "sandbox root {} must be an absolute path below / with no .. made of \
                 letters, digits and . - _ /",
                self.root_dir
            );
        }
        let root = Path::new(&self.root_dir);
        if !root.is_dir() {
            anyhow::bail!(
                "sandbox root {} is not an existing directory",
                self.root_dir
            );
        }
        Ok(())
    }
}
