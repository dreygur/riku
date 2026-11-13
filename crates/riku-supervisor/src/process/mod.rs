//! Process management module for the supervisor.
//!
//! Handles spawning, monitoring, health checks, and managing application processes.

pub mod generation;
pub mod health_check;
mod health_interval;
mod health_probe;
pub mod isolation;
pub(crate) mod lifecycle_events;
mod log_capture;
pub mod orchestration;
pub mod pid_file;
mod restart;
pub mod spawn;
pub mod spawned;
pub mod stop;
#[cfg(test)]
pub(crate) mod test_support;
mod worker_cgroup;

pub use generation::{AppGeneration, GenerationStatus};
pub use spawned::SpawnedProcess;

use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::cgroups::CgroupTree;
use crate::resource_limits::ResourceLimits;
use crate::stats::StatsManager;
use lifecycle_events::EventTarget;
use orchestration::{new_probe_results, ProbeResults};

/// Manages the lifecycle of application processes.
pub struct ProcessManager {
    pub(super) processes: HashMap<String, SpawnedProcess>, // Key: app_name-worker_kind-ordinal
    pub(super) stats: StatsManager,
    resource_limits: ResourceLimits,
    /// Versioned deployment generations, keyed by canonical process_id.
    pub(super) generations: HashMap<String, Vec<AppGeneration>>,
    /// Outcomes written by background health-probe threads, drained once
    /// per tick by `reconcile_generations`.
    pub(super) probe_results: ProbeResults,
    /// Structured rollback/promotion notifications waiting to be pushed
    /// onto the metrics SSE broadcast channel.
    pub(super) deployment_events: Vec<String>,
    /// Where each spawned process gets a `pid_file` marker. `None` writes
    /// no markers, which only tests rely on.
    pub(super) pid_dir: Option<PathBuf>,
    /// Where crash lifecycle events go.
    pub(super) events: EventTarget,
    /// The cgroup the supervisor took over at startup. `None` when it is
    /// not delegated, and then isolated workers refuse to start.
    cgroup_tree: Option<CgroupTree>,
}

impl ProcessManager {
    /// Create a new process manager.
    pub fn new() -> Result<Self> {
        let resource_limits = ResourceLimits::from_env();

        tracing::info!(
            "ProcessManager initialized with resource limits: {}",
            resource_limits.summary()
        );

        Ok(ProcessManager {
            processes: HashMap::new(),
            stats: StatsManager::new(),
            resource_limits,
            generations: HashMap::new(),
            probe_results: new_probe_results(),
            deployment_events: Vec::new(),
            pid_dir: None,
            events: EventTarget::default(),
            cgroup_tree: None,
        })
    }

    /// Send crash lifecycle events to `events` instead of the riku tree the
    /// environment names.
    #[cfg(test)]
    pub(crate) fn with_events(mut self, events: EventTarget) -> Self {
        self.events = events;
        self
    }

    /// Keep a marker per spawned process in `dir` (see `pid_file`).
    pub fn with_pid_dir(mut self, dir: PathBuf) -> Self {
        self.pid_dir = Some(dir);
        self
    }

    /// Remove the markers a previous supervisor left behind. Call once,
    /// before the first spawn.
    pub fn clear_stale_pid_markers(&self) -> std::io::Result<()> {
        match &self.pid_dir {
            Some(dir) => pid_file::clear_markers(dir),
            None => Ok(()),
        }
    }

    /// Get the number of managed processes.
    pub fn get_process_count(&self) -> usize {
        self.processes.len()
    }

    /// Get a clone of the resource limits configuration.
    pub fn get_resource_limits(&self) -> ResourceLimits {
        self.resource_limits.clone()
    }

    /// Get a reference to the stats manager.
    pub fn stats(&self) -> &StatsManager {
        &self.stats
    }

    /// Whether a process slot is currently spawned and tracked.
    pub fn is_managed(&self, process_id: &str) -> bool {
        self.processes.contains_key(process_id)
    }

    /// Get a mutable reference to the stats manager.
    pub fn stats_mut(&mut self) -> &mut StatsManager {
        &mut self.stats
    }
}
