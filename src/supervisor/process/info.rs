//! Process listing and information retrieval.

use crate::supervisor::stats::ProcessStatus;

use super::ProcessManager;

/// Process information for CLI display.
#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct ProcessInfo {
    pub process_id: String,
    pub app: String,
    pub kind: String,
    pub ordinal: u32,
    pub pid: u32,
    pub status: ProcessStatus,
    pub restart_count: u32,
}

impl ProcessManager {
    /// Whether a process slot is currently spawned and tracked.
    pub fn is_managed(&self, process_id: &str) -> bool {
        self.processes.contains_key(process_id)
    }

    /// Get a list of all managed processes with their status.
    #[allow(dead_code)]
    pub fn list_processes(&self) -> Vec<ProcessInfo> {
        self.processes
            .iter()
            .map(|(id, process)| {
                let stats = self.stats.get_process_stats(id);
                ProcessInfo {
                    process_id: id.to_string(),
                    app: process.config.worker.app.clone(),
                    kind: process.config.worker.kind.clone(),
                    ordinal: process.config.worker.ordinal,
                    pid: process.pid_as_u32(),
                    status: stats
                        .map(|s| s.status.clone())
                        .unwrap_or(ProcessStatus::Running),
                    restart_count: process.restart_count,
                }
            })
            .collect()
    }

    /// Get processes for a specific app.
    #[allow(dead_code)]
    pub fn get_app_processes(&self, app_name: &str) -> Vec<ProcessInfo> {
        self.list_processes()
            .into_iter()
            .filter(|p| p.app == app_name)
            .collect()
    }
}
