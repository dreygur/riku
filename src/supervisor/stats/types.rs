//! Data types for process statistics and metrics.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

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
    pub requests_total: u64,
    pub requests_per_second: f64,
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
            requests_total: 0,
            requests_per_second: 0.0,
        }
    }
}

/// Process status enum.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum ProcessStatus {
    Starting,
    Running,
    Stopped,
    Crashed,
    Restarting,
}

impl std::fmt::Display for ProcessStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProcessStatus::Starting => write!(f, "starting"),
            ProcessStatus::Running => write!(f, "running"),
            ProcessStatus::Stopped => write!(f, "stopped"),
            ProcessStatus::Crashed => write!(f, "crashed"),
            ProcessStatus::Restarting => write!(f, "restarting"),
        }
    }
}

/// Health check status.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
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
#[allow(dead_code)]
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
