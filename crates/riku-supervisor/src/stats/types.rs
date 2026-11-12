//! Data types for process statistics and metrics.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Facts about a container process reported by the container engine.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ContainerFacts {
    /// Engine id of the container.
    pub id: String,
    /// Reference the container was created from.
    pub image: String,
    /// Engine state word.
    pub state: String,
    /// Some only for an exited container.
    pub exit_code: Option<i32>,
    /// Percent of one core from the last stats sample.
    pub cpu_percent: f64,
}

/// Statistics for a single process.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProcessStats {
    pub process_id: String,
    pub app: String,
    pub kind: String,
    pub ordinal: u32,
    pub pid: Option<u32>,
    pub status: ProcessStatus,
    pub started_at: Option<DateTime<Utc>>,
    pub last_health_check: Option<DateTime<Utc>>,
    pub health_check_status: HealthStatus,
    pub restart_count: u32,
    pub last_restart_at: Option<DateTime<Utc>>,
    pub cpu_time_ms: u64,
    pub memory_bytes: u64,
    /// The worker's memory limit: its cgroup `memory.max`, else the limit
    /// its config sets. `None` when it has none.
    #[serde(default)]
    pub memory_limit_bytes: Option<u64>,
    pub requests_total: u64,
    pub requests_per_second: f64,
    /// Facts reported by the container engine (None for Procfile processes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container: Option<ContainerFacts>,
}

impl Default for ProcessStats {
    fn default() -> Self {
        ProcessStats {
            process_id: String::new(),
            app: String::new(),
            kind: String::new(),
            ordinal: 0,
            pid: None,
            status: ProcessStatus::Starting,
            started_at: None,
            last_health_check: None,
            health_check_status: HealthStatus::Unknown,
            restart_count: 0,
            last_restart_at: None,
            cpu_time_ms: 0,
            memory_bytes: 0,
            memory_limit_bytes: None,
            requests_total: 0,
            requests_per_second: 0.0,
            container: None,
        }
    }
}

/// Process status enum.
///
/// `rename_all = "snake_case"` keeps the JSON wire format aligned with the
/// `Display` impl below (and with what dashboard/lib/types.ts's STATUS_MAP
/// expects): without it, serde's default PascalCase variant names
/// ("Running", "Crashed", ...) silently fail every lookup in that map and
/// every worker renders as "stopped" regardless of its real status.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ProcessStatus {
    Starting,
    Running,
    Stopped,
    Crashed,
    Restarting,
    /// The worker's cgroup `memory.max` was exceeded and the kernel OOM
    /// killer terminated it (detected via `memory.events: oom_kill`).
    OomKilled,
}

impl std::fmt::Display for ProcessStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProcessStatus::Starting => write!(f, "starting"),
            ProcessStatus::Running => write!(f, "running"),
            ProcessStatus::Stopped => write!(f, "stopped"),
            ProcessStatus::Crashed => write!(f, "crashed"),
            ProcessStatus::Restarting => write!(f, "restarting"),
            ProcessStatus::OomKilled => write!(f, "oom_killed"),
        }
    }
}

/// Health check status.
///
/// `rename_all = "snake_case"`: see `ProcessStatus` doc comment above for
/// why; dashboard/lib/types.ts's HEALTH_MAP has the same lowercase-key
/// assumption.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    Unknown,
    Healthy,
    Unhealthy,
    Timeout,
    Error(String),
}

impl std::fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HealthStatus::Unknown => write!(f, "unknown"),
            HealthStatus::Healthy => write!(f, "healthy"),
            HealthStatus::Unhealthy => write!(f, "unhealthy"),
            HealthStatus::Timeout => write!(f, "timeout"),
            HealthStatus::Error(e) => write!(f, "error: {}", e),
        }
    }
}

/// Aggregated statistics for an application.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AppStats {
    pub app: String,
    pub total_processes: u32,
    pub running_processes: u32,
    pub healthy_processes: u32,
    pub total_restarts: u32,
    pub total_memory_bytes: u64,
    pub total_cpu_time_ms: u64,
    pub processes: Vec<ProcessStats>,
    pub last_updated: DateTime<Utc>,
}
