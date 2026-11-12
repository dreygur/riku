//! Data types representing supervisor state dumps and worker entries.

use serde::Serialize;
use std::collections::BTreeMap;

use super::route::RouteState;
use crate::supervisor::stats::ProcessStatus;

/// Product name and version of the container engine, for operator visibility.
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct EngineInfo {
    pub name: String,
    pub version: String,
}

/// Execution unit kind for a worker entry.
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UnitKind {
    Process,
    Container,
}

/// High-level lock state of an app's deployment mutex.
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LockState {
    Free,
    Held,
}

/// The entire supervisor state dump across all apps and the container engine.
#[derive(Serialize)]
pub struct StateDump {
    pub generated_at: u64,
    pub riku_version: &'static str,
    pub supervisor_uptime_seconds: Option<u64>,
    /// Why the uptime was not read (run as root), left out otherwise. A
    /// `null` uptime without it means no supervisor holds the pid file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supervisor_uptime_unavailable: Option<&'static str>,
    pub engine: Option<EngineInfo>,
    pub apps: Vec<AppStateEntry>,
}

/// State snapshot for a single app.
#[derive(Serialize)]
pub struct AppStateEntry {
    pub app: String,
    pub deploy_lock: LockState,
    /// Allowlisted routing/port env vars only: secrets are never dumped.
    pub routing: BTreeMap<String, String>,
    pub route: RouteState,
    pub workers: Vec<WorkerStateEntry>,
    /// Whether any deploy of the app recorded a release. False for an app
    /// that was created but whose deploys all failed.
    pub released: bool,
    /// Process kinds the scale API takes for this Procfile app, sorted: the
    /// long running kinds in `SCALING` and in `workers`. Empty for a
    /// container app, whose services are its workers' kinds.
    pub process_kinds: Vec<String>,
    /// Why the app's config (riku.toml or ENV) could not be read; `routing`
    /// is then empty. Left out of the JSON when the config is fine.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_error: Option<String>,
}

/// State snapshot for an individual worker or container copy.
#[derive(Serialize)]
pub struct WorkerStateEntry {
    pub process_id: String,
    pub kind: String,
    pub ordinal: u32,
    pub pid: Option<u32>,
    pub status: ProcessStatus,
    pub restart_count: u32,
    pub memory_bytes: u64,
    /// The worker's memory limit (cgroup `memory.max` or the configured
    /// limit); `null` when it has none, and always for a container.
    pub memory_limit_bytes: Option<u64>,
    pub cpu_time_ms: u64,
    pub unit: UnitKind,
    pub container_id: Option<String>,
    pub image: Option<String>,
    pub state_detail: Option<String>,
    pub cpu_percent: Option<f64>,
}
