//! Fixtures shared by the process module's test files.
//!
//! `spawn.rs` and `health_check.rs` both need a minimal `WorkerConfig` to
//! spawn a real (short-lived) process against, and the daemon tests need to
//! look up a running worker's PID by its canonical `process_id`. Kept in one
//! place instead of copies drifting apart.

use super::ProcessManager;
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

/// Minimal process identity used by tests that need to locate a running
/// worker's PID by its canonical `process_id`.
#[derive(Clone, Debug)]
pub struct ProcessInfo {
    pub process_id: String,
    pub pid: u32,
}

impl ProcessManager {
    /// List every managed process with its current PID.
    pub fn list_processes(&self) -> Vec<ProcessInfo> {
        self.processes
            .iter()
            .map(|(id, process)| ProcessInfo {
                process_id: id.to_string(),
                pid: process.pid_as_u32(),
            })
            .collect()
    }
}
