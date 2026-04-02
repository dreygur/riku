//! Process stopping and termination logic for the ProcessManager.

use anyhow::Result;
use std::thread;
use std::time::Duration;

use super::ProcessManager;

impl ProcessManager {
    /// Stop a specific process by its ID.
    pub(super) fn stop_process_by_id(&mut self, process_id: &str) -> Result<()> {
        if let Some(mut process) = self.processes.remove(process_id) {
            tracing::info!("Stopping process: {} (PID: {})", process_id, process.pid);

            // Update stats
            self.stats.mark_stopped(process_id);

            // Try graceful shutdown with SIGTERM
            process.terminate()?;

            // Wait for graceful shutdown using the configured grace_period (in seconds).
            // Poll every 100 ms so we exit promptly when the process dies.
            let grace_period = process.config.options.grace_period;
            let deadline = Duration::from_secs(grace_period);
            let poll_interval = Duration::from_millis(100);
            let mut elapsed = Duration::ZERO;
            while process.is_running() && elapsed < deadline {
                thread::sleep(poll_interval);
                elapsed += poll_interval;
            }

            // If still running after the grace period, force kill with SIGKILL
            if process.is_running() {
                tracing::warn!(
                    "Process {} didn't respond to SIGTERM within {}s, sending SIGKILL",
                    process_id, grace_period
                );
                process.kill()?;

                thread::sleep(Duration::from_millis(500));
            }

            tracing::info!("Process {} stopped", process_id);
        }
        Ok(())
    }

    /// Stop all processes for a specific app.
    pub fn stop_app_processes(&mut self, app_name: &str) -> Result<()> {
        let processes_to_remove: Vec<String> = self
            .processes
            .keys()
            .filter(|id: &&String| id.starts_with(&format!("{}-", app_name)))
            .cloned()
            .collect();

        for process_id in processes_to_remove {
            self.stop_process_by_id(&process_id)?;
        }
        Ok(())
    }

    /// Stop all managed processes, respecting each process's configured grace_period.
    pub fn stop_all_processes(&mut self) -> Result<()> {
        let process_ids: Vec<String> = self.processes.keys().cloned().collect();

        for process_id in process_ids {
            self.stop_process_by_id(&process_id)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::supervisor::config::{WorkerConfig, WorkerInfo, WorkerOptions};
    use crate::supervisor::process::ProcessManager;
    use std::collections::HashMap;
    use tempfile::TempDir;

    fn sleep_config(tmp: &TempDir) -> WorkerConfig {
        let log = tmp.path().join("out.log");
        WorkerConfig {
            worker: WorkerInfo {
                app: "testapp".to_string(),
                kind: "web".to_string(),
                command: "sleep 60".to_string(),
                ordinal: 1,
            },
            env: HashMap::new(),
            options: WorkerOptions {
                working_dir: tmp.path().to_str().unwrap().to_string(),
                log_file: log.to_str().unwrap().to_string(),
                uid: None,
                gid: None,
                timeout: 30,
                // short grace period so the test doesn't wait 30 s
                grace_period: 1,
                max_restarts: 3,
                health_check: None,
            },
        }
    }

    #[test]
    fn test_stop_app_processes_removes_them_from_manager() {
        let tmp = TempDir::new().unwrap();
        let config = sleep_config(&tmp);

        let mut pm = ProcessManager::new().unwrap();
        pm.spawn_process(&config).expect("spawn should succeed");
        assert_eq!(pm.get_process_count(), 1);

        pm.stop_app_processes("testapp")
            .expect("stop_app_processes should not fail");

        assert_eq!(
            pm.get_process_count(),
            0,
            "all processes for 'testapp' should be removed after stop"
        );
    }

    #[test]
    fn test_stop_all_processes_clears_manager() {
        let tmp = TempDir::new().unwrap();
        let mut config = sleep_config(&tmp);

        let mut pm = ProcessManager::new().unwrap();

        // Spawn two different workers.
        pm.spawn_process(&config).expect("spawn 1");
        config.worker.ordinal = 2;
        pm.spawn_process(&config).expect("spawn 2");
        assert_eq!(pm.get_process_count(), 2);

        pm.stop_all_processes().expect("stop_all_processes should not fail");
        assert_eq!(pm.get_process_count(), 0, "all processes should be removed");
    }

    #[test]
    fn test_stop_nonexistent_process_is_noop() {
        let tmp = TempDir::new().unwrap();
        let _ = tmp; // keep alive

        let mut pm = ProcessManager::new().unwrap();
        // Stopping a process that was never registered must not panic or error.
        pm.stop_app_processes("nonexistent-app")
            .expect("stopping a nonexistent app should be a silent no-op");
    }
}
