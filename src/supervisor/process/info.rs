//! Process listing and information retrieval.

use super::ProcessManager;

/// Minimal process identity used by tests that need to locate a running
/// worker's PID by its canonical `process_id`.
#[cfg(test)]
#[derive(Clone, Debug)]
pub struct ProcessInfo {
    pub process_id: String,
    pub pid: u32,
}

impl ProcessManager {
    /// Whether a process slot is currently spawned and tracked.
    pub fn is_managed(&self, process_id: &str) -> bool {
        self.processes.contains_key(process_id)
    }

    /// Get a list of all managed processes with their current PID.
    #[cfg(test)]
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
