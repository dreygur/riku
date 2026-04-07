//! Hot-reload (zero-downtime restart) for managed processes.

use anyhow::Result;
use std::thread;
use std::time::Duration;

use super::ProcessManager;

impl ProcessManager {
    /// Hot reload a process - graceful restart with zero downtime.
    ///
    /// Spawns the new process under a temporary key, gives it time to start,
    /// then gracefully shuts down the old process and renames the new entry
    /// to the canonical process_id. This avoids the race where spawn_process()
    /// would stop the old process automatically when the key already exists.
    #[allow(dead_code)]
    pub fn hot_reload_process(&mut self, process_id: &str) -> Result<()> {
        let (config, old_pid) = match self.processes.get(process_id) {
            Some(p) => (p.config.clone(), p.pid_as_u32()),
            None => return Ok(()),
        };

        tracing::info!("Hot reloading process {} (PID: {})", process_id, old_pid);

        // Spawn new process under a temporary key so spawn_process() does not
        // automatically stop the old entry (which lives under process_id).
        let temp_id = format!("{}__hot_new", process_id);
        let mut new_config = config.clone();
        // Temporarily change the ordinal to produce a different process_id key.
        // We will rename it back after the old process is stopped.
        new_config.worker.ordinal = new_config.worker.ordinal.wrapping_add(u32::MAX / 2);
        self.spawn_process(&new_config)?;
        let new_temp_key = format!(
            "{}-{}-{}",
            new_config.worker.app, new_config.worker.kind, new_config.worker.ordinal
        );

        // Give the new process time to start and become ready.
        thread::sleep(Duration::from_millis(500));

        // Remove the old process entry (Drop triggers SIGTERM → wait → SIGKILL).
        if let Some(mut old_process) = self.processes.remove(process_id) {
            let grace = old_process.config.options.grace_period;
            old_process.terminate()?;
            let deadline = Duration::from_secs(grace);
            let poll = Duration::from_millis(100);
            let mut elapsed = Duration::ZERO;
            while old_process.is_running() && elapsed < deadline {
                thread::sleep(poll);
                elapsed += poll;
            }
            if old_process.is_running() {
                old_process.kill()?;
                thread::sleep(Duration::from_millis(100));
            }
        }

        // Rename the new process entry from the temp key back to the canonical key.
        if let Some(mut new_process) = self.processes.remove(&new_temp_key) {
            // Restore the original ordinal in the config so stats and future
            // operations use the correct process_id.
            new_process.config.worker.ordinal = config.worker.ordinal;
            self.processes.insert(process_id.to_string(), new_process);
        }

        // Also clean up the temp_id entry if it was inserted under yet another key.
        self.processes.remove(&temp_id);

        tracing::info!("Hot reload complete for {}", process_id);
        Ok(())
    }

    /// Hot reload all processes for an app.
    #[allow(dead_code)]
    pub fn hot_reload_app(&mut self, app_name: &str) -> Result<()> {
        let process_ids: Vec<String> = self
            .processes
            .keys()
            .filter(|id: &&String| id.starts_with(&format!("{}-", app_name)))
            .cloned()
            .collect();

        for process_id in process_ids {
            self.hot_reload_process(&process_id)?;
        }

        Ok(())
    }
}
