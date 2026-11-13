//! Restarting crashed or unhealthy processes, and dropping the ones that
//! ran out of restarts.

use anyhow::Result;

use super::ProcessManager;

impl ProcessManager {
    /// Third pass: restart processes that need it; entries prefixed
    /// "__remove__" are permanently failed processes that must be removed
    /// without restarting. Limits concurrent restarts to prevent a
    /// thundering herd.
    // Explicit match over `?` per project convention (.claude/CLAUDE.md rule 16).
    #[allow(clippy::question_mark)]
    pub(super) fn restart_or_remove_processes(&mut self, to_restart: Vec<String>) -> Result<()> {
        const MAX_RESTARTS_PER_CYCLE: usize = 5;
        let mut restarts_this_cycle = 0;

        for process_id in to_restart {
            if let Some(id) = process_id.strip_prefix("__remove__") {
                // Fire app.failed before removing: this is the one case
                // restart_process() never reaches, and it's the case an
                // admin most needs to hear about (riku has given up, not
                // just self-healed).
                if let Some(p) = self.processes.get(id) {
                    self.events.app_failed(
                        p.config.worker.app.clone(),
                        id.to_string(),
                        p.last_exit_code,
                        p.restart_count,
                        p.config.options.max_restarts,
                    );
                }
                // Remove the dead entry so Drop reaps the zombie child.
                self.processes.remove(id);
                tracing::error!(
                    "Process {} permanently failed; removed from supervision",
                    id
                );
            } else {
                // Stagger restarts to prevent system overload
                if restarts_this_cycle < MAX_RESTARTS_PER_CYCLE {
                    if let Err(e) = self.restart_process(&process_id) {
                        return Err(e);
                    }
                    restarts_this_cycle += 1;
                } else {
                    tracing::debug!(
                        "Deferring restart of {} to next cycle (throttling)",
                        process_id
                    );
                }
            }
        }

        Ok(())
    }

    /// Restart a specific process.
    pub(super) fn restart_process(&mut self, process_id: &str) -> Result<()> {
        tracing::info!("Restarting process: {}", process_id);

        // Capture config, current restart count, and the crash's exit code
        // before removing the process.
        let (config, prev_restart_count, exit_code) = match self.processes.get(process_id) {
            Some(p) => (Some(p.config.clone()), p.restart_count, p.last_exit_code),
            None => (None, 0, None),
        };

        if let Some(config) = config {
            // Mark as restarting in stats
            self.stats.mark_restarting(process_id);

            // Remove the old process entry (terminates it via Drop).
            self.processes.remove(process_id);

            // Spawn a fresh process and then update its restart counter so that
            // the exponential backoff keeps growing across successive crashes.
            self.spawn_process(&config)?;

            let new_restart_count = prev_restart_count + 1;
            if let Some(new_process) = self.processes.get_mut(process_id) {
                new_process.restart_count = new_restart_count;
                new_process.last_restart = std::time::Instant::now();
            }

            self.events.app_restarted(
                config.worker.app,
                process_id.to_string(),
                exit_code,
                new_restart_count,
            );
        }

        Ok(())
    }
}
