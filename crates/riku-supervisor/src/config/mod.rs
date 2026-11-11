//! Worker configuration module for the supervisor.
//!
//! Defines the structure for TOML-based worker configurations.

use riku_util::file_names::worker::WorkerName;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

mod ownership;
pub use ownership::app_worker_configs;

#[cfg(test)]
mod tests;

/// Reads `$key` out of a worker's `[env]` map (`$env: &HashMap<String, String>`),
/// parses it as `$ty`, and falls back to `$default` (a `fn() -> $ty`) if the key
/// is missing or fails to parse. Collapses the
/// `env.get(key).and_then(|v| v.parse().ok()).unwrap_or_else(default)` shape
/// that otherwise repeats for every `RIKU_*` worker setting.
macro_rules! parse_env_or {
    ($env:expr, $key:literal, $ty:ty, $default:expr) => {
        $env.get($key)
            .and_then(|v| v.parse::<$ty>().ok())
            .unwrap_or_else($default)
    };
}

/// Same lookup as [`parse_env_or!`], but yields `None` (rather than a default)
/// when the key is missing or unparsable: for settings that are opt-in with
/// no meaningful default.
macro_rules! parse_env_opt {
    ($env:expr, $key:literal, $ty:ty) => {
        $env.get($key).and_then(|v| v.parse::<$ty>().ok())
    };
}

mod limits;
pub use limits::{Sandbox, WorkerLimits};

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

impl WorkerInfo {
    /// The process id and config file name this worker runs under.
    pub fn name(&self) -> WorkerName {
        WorkerName::new(&self.app, &self.kind, self.ordinal)
    }
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
    /// cgroup limits. `Some` puts the worker in its own cgroup with these
    /// limits, without any sandbox.
    #[serde(default)]
    pub limits: Option<WorkerLimits>,
    /// The opt in chroot and namespace sandbox. See [`Sandbox`] for why it
    /// is off by default.
    #[serde(default, rename = "isolation")]
    pub sandbox: Option<Sandbox>,
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
    riku_config::RIKU_WORKER_TIMEOUT
}

fn default_grace_period() -> u64 {
    riku_config::RIKU_WORKER_GRACE_PERIOD
}

fn default_max_restarts() -> u32 {
    riku_config::RIKU_MAX_RESTARTS
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
                limits: None,
                sandbox: None,
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
    let timeout = parse_env_or!(env, "RIKU_WORKER_TIMEOUT", u64, default_timeout);
    let grace_period = parse_env_or!(env, "RIKU_WORKER_GRACE_PERIOD", u64, default_grace_period);
    let max_restarts = parse_env_or!(env, "RIKU_MAX_RESTARTS", u32, default_max_restarts);

    // Add RIKU_BIND_ADDRESS to worker env if not already set
    if !env.contains_key("RIKU_BIND_ADDRESS") {
        env.insert("RIKU_BIND_ADDRESS".to_string(), "127.0.0.1".to_string());
    }

    // Read health check settings from environment
    let health_check = env.get("RIKU_HEALTH_CHECK_URL").map(|_| HealthCheck {
        url: parse_env_or!(
            env,
            "RIKU_HEALTH_CHECK_URL",
            String,
            default_health_check_url
        ),
        interval: parse_env_or!(
            env,
            "RIKU_HEALTH_CHECK_INTERVAL",
            u64,
            default_health_check_interval
        ),
        timeout: parse_env_or!(
            env,
            "RIKU_HEALTH_CHECK_TIMEOUT",
            u64,
            default_health_check_timeout
        ),
        retries: parse_env_or!(
            env,
            "RIKU_HEALTH_CHECK_RETRIES",
            u32,
            default_health_check_retries
        ),
    });

    let limits = WorkerLimits::from_env(&env);
    let sandbox = Sandbox::from_env(&env);

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
            limits,
            sandbox,
        },
    }
}
