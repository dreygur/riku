//! Worker configuration module for the supervisor.
//!
//! Defines the structure for TOML-based worker configurations.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[cfg(test)]
mod tests;

/// The main worker configuration structure stored in TOML files.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WorkerConfig {
    pub worker: WorkerInfo,
    pub env: HashMap<String, String>,
    pub options: WorkerOptions,
}

/// Information about the worker process.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WorkerInfo {
    pub app: String,
    pub kind: String,
    pub command: String,
    pub ordinal: u32,
}

/// Options for the worker process.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WorkerOptions {
    pub working_dir: String,
    pub log_file: String,
    #[serde(default)]
    pub uid: Option<String>,
    #[serde(default)]
    pub gid: Option<String>,
    #[serde(default = "default_timeout")]
    pub timeout: u64,
    #[serde(default = "default_grace_period")]
    pub grace_period: u64,
    #[serde(default = "default_max_restarts")]
    pub max_restarts: u32,
    #[serde(default)]
    pub health_check: Option<HealthCheck>,
    /// Opt-in kernel-level isolation (PID/net/mount namespaces + cgroup v2
    /// limits). Disabled by default: `CLONE_NEWNET` restricts the worker to
    /// loopback only, which breaks workers that need to reach a database
    /// or other service over the host network unless that's provisioned
    /// separately (e.g. a veth pair). Enable only for workers that are
    /// fully self-contained or for which that tradeoff has been accepted.
    #[serde(default)]
    pub isolation: Option<IsolationOptions>,
}

/// Kernel-level isolation settings for a worker, read from TOML/env.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct IsolationOptions {
    /// Directory to `pivot_root` the worker into. Must contain everything
    /// the worker needs (app code, libraries, `/proc`, `/dev`, etc).
    pub root_dir: String,
    /// Hard memory ceiling in bytes (cgroup v2 `memory.max`).
    #[serde(default)]
    pub max_memory_bytes: Option<u64>,
    /// CPU quota in microseconds per `cpu_period_us` (cgroup v2 `cpu.max`).
    /// `None` leaves CPU unconstrained.
    #[serde(default)]
    pub cpu_quota_us: Option<u64>,
    /// CPU accounting period in microseconds.
    #[serde(default = "default_cpu_period_us")]
    pub cpu_period_us: u64,
    /// Maximum number of tasks (processes/threads) the worker's cgroup may
    /// contain at once (cgroup v2 `pids.max`). `None` leaves it unlimited.
    /// This is the correct, per-worker-scoped replacement for
    /// `RIKU_MAX_PROCESSES`/`RLIMIT_NPROC`, which counts every process
    /// owned by the real UID system-wide rather than just this worker's
    /// subtree.
    #[serde(default)]
    pub max_pids: Option<u64>,
}

fn default_cpu_period_us() -> u64 {
    100_000
}

/// Health check configuration for a worker.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HealthCheck {
    #[serde(default = "default_health_check_url")]
    pub url: String,
    #[serde(default = "default_health_check_interval")]
    pub interval: u64,
    #[serde(default = "default_health_check_timeout")]
    pub timeout: u64,
    #[serde(default = "default_health_check_retries")]
    pub retries: u32,
}

fn default_health_check_url() -> String {
    "/health".to_string()
}

fn default_health_check_interval() -> u64 {
    30
}

fn default_health_check_timeout() -> u64 {
    5
}

fn default_health_check_retries() -> u32 {
    3
}

fn default_timeout() -> u64 {
    crate::config::RIKU_WORKER_TIMEOUT
}

fn default_grace_period() -> u64 {
    crate::config::RIKU_WORKER_GRACE_PERIOD
}

fn default_max_restarts() -> u32 {
    crate::config::RIKU_MAX_RESTARTS
}

impl Default for WorkerConfig {
    fn default() -> Self {
        WorkerConfig {
            worker: WorkerInfo {
                app: String::new(),
                kind: String::new(),
                command: String::new(),
                ordinal: 0,
            },
            env: HashMap::new(),
            options: WorkerOptions {
                working_dir: String::new(),
                log_file: String::new(),
                uid: None,
                gid: None,
                timeout: default_timeout(),
                grace_period: default_grace_period(),
                max_restarts: default_max_restarts(),
                health_check: None,
                isolation: None,
            },
        }
    }
}

/// Create a worker config from app name, kind, command, and environment.
/// Reads RIKU_* environment variables for worker management settings.
pub fn create_worker_config(
    app: &str,
    kind: &str,
    command: &str,
    ordinal: u32,
    mut env: HashMap<String, String>,
    working_dir: &str,
    log_file: &str,
) -> WorkerConfig {
    // Read RIKU_* settings from environment with defaults
    let timeout = env
        .get("RIKU_WORKER_TIMEOUT")
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or_else(default_timeout);

    let grace_period = env
        .get("RIKU_WORKER_GRACE_PERIOD")
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or_else(default_grace_period);

    let max_restarts = env
        .get("RIKU_MAX_RESTARTS")
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or_else(default_max_restarts);

    // Add BIND_ADDRESS to worker env if not already set
    if !env.contains_key("BIND_ADDRESS") {
        env.insert("BIND_ADDRESS".to_string(), "127.0.0.1".to_string());
    }

    // Read health check settings from environment
    let health_check = env.get("RIKU_HEALTH_CHECK_URL").map(|_| HealthCheck {
        url: env
            .get("RIKU_HEALTH_CHECK_URL")
            .cloned()
            .unwrap_or_else(default_health_check_url),
        interval: env
            .get("RIKU_HEALTH_CHECK_INTERVAL")
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or_else(default_health_check_interval),
        timeout: env
            .get("RIKU_HEALTH_CHECK_TIMEOUT")
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or_else(default_health_check_timeout),
        retries: env
            .get("RIKU_HEALTH_CHECK_RETRIES")
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or_else(default_health_check_retries),
    });

    // Isolation is opt-in: only enabled when RIKU_ISOLATION_ROOT is set,
    // since CLONE_NEWNET cuts the worker off from the host network beyond
    // loopback (see IsolationOptions docs).
    let isolation = env
        .get("RIKU_ISOLATION_ROOT")
        .map(|root_dir| IsolationOptions {
            root_dir: root_dir.clone(),
            max_memory_bytes: env
                .get("RIKU_ISOLATION_MAX_MEMORY_MB")
                .and_then(|v| v.parse::<u64>().ok())
                .map(|mb| mb * 1024 * 1024),
            cpu_quota_us: env
                .get("RIKU_ISOLATION_CPU_QUOTA_US")
                .and_then(|v| v.parse::<u64>().ok()),
            cpu_period_us: env
                .get("RIKU_ISOLATION_CPU_PERIOD_US")
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or_else(default_cpu_period_us),
            max_pids: env
                .get("RIKU_ISOLATION_MAX_PIDS")
                .and_then(|v| v.parse::<u64>().ok()),
        });

    WorkerConfig {
        worker: WorkerInfo {
            app: app.to_string(),
            kind: kind.to_string(),
            command: command.to_string(),
            ordinal,
        },
        env,
        options: WorkerOptions {
            working_dir: working_dir.to_string(),
            log_file: log_file.to_string(),
            uid: None,
            gid: None,
            timeout,
            grace_period,
            max_restarts,
            health_check,
            isolation,
        },
    }
}
