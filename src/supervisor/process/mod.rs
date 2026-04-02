//! Process management module for the supervisor.
//!
//! Handles spawning, monitoring, health checks, and managing application processes.

pub mod health_check;
pub mod hot_reload;
pub mod info;
pub mod spawn;
pub mod spawned;
pub mod stop;

pub use spawned::SpawnedProcess;

use anyhow::Result;
use std::collections::HashMap;

use crate::supervisor::resource_limits::ResourceLimits;
use crate::supervisor::stats::StatsManager;

/// Manages the lifecycle of application processes.
pub struct ProcessManager {
    pub(super) processes: HashMap<String, SpawnedProcess>, // Key: app_name-worker_kind-ordinal
    pub(super) stats: StatsManager,
    resource_limits: ResourceLimits,
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
