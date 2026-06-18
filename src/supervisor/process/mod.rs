//! Process management module for the supervisor.
//!
//! Handles spawning, monitoring, health checks, and managing application processes.

pub mod generation;
pub mod health_check;
pub mod hot_reload;
pub mod info;
pub mod isolation;
pub mod orchestration;
pub mod spawn;
pub mod spawned;
pub mod stop;

#[allow(unused_imports)]
pub use generation::{AppGeneration, GenerationStatus};
pub use spawned::SpawnedProcess;

use anyhow::Result;
use std::collections::HashMap;

use crate::supervisor::resource_limits::ResourceLimits;
use crate::supervisor::stats::StatsManager;
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
        })
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
    #[allow(dead_code)]
    pub fn stats(&self) -> &StatsManager {
        &self.stats
    }

    /// Get a mutable reference to the stats manager.
    #[allow(dead_code)]
    pub fn stats_mut(&mut self) -> &mut StatsManager {
        &mut self.stats
    }
}
