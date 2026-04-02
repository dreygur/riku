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
