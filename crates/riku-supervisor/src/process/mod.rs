//! Process management module for the supervisor.
//!
//! Handles spawning, monitoring, health checks, and managing application processes.

pub mod generation;
pub mod health_check;
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

use crate::resource_limits::ResourceLimits;
use crate::stats::StatsManager;
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
    pub fn stats(&self) -> &StatsManager {
        &self.stats
    }

    /// Get a mutable reference to the stats manager.
    pub fn stats_mut(&mut self) -> &mut StatsManager {
        &mut self.stats
    }
}

/// Shared by `spawn.rs` and `health_check.rs`'s test modules, which both
/// need a minimal `WorkerConfig` to spawn a real (short-lived) process
/// against — kept in one place instead of two copies drifting apart.
#[cfg(test)]
pub(crate) mod test_support {
    use crate::config::{WorkerConfig, WorkerInfo, WorkerOptions};
    use std::collections::HashMap;

    pub fn minimal_config(command: &str, working_dir: &str, log_file: &str) -> WorkerConfig {
        WorkerConfig {
            worker: WorkerInfo {
                app: "testapp".to_string(),
                kind: "web".to_string(),
                command: command.to_string(),
                ordinal: 1,
            },
            env: HashMap::new(),
            options: WorkerOptions {
                working_dir: working_dir.to_string(),
                log_file: log_file.to_string(),
                uid: None,
                gid: None,
                timeout: 30,
                grace_period: 2,
                max_restarts: 3,
                health_check: None,
                isolation: None,
            },
        }
    }
}
