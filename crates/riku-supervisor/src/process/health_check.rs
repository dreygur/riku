//! Health check and process monitoring for the ProcessManager.

use anyhow::Result;
use std::time::Duration;

use crate::config::HealthCheck;
use crate::stats::{get_process_resources, HealthStatus};

use super::ProcessManager;

impl ProcessManager {
    /// Check the status of all managed processes, perform health checks, and restart crashed ones.
    pub fn check_processes(&mut self) -> Result<()> {
        let mut to_restart = Vec::new();
        let mut health_checks: Vec<(String, HealthCheck)> = Vec::new();

        // Generations under active probing are owned exclusively by the
        // orchestrator (`reconcile_generations` + the probe thread's circuit
        // breaker) — skip them here so the two restart paths never race.
        let probing_keys: std::collections::HashSet<String> = self
            .generations
            .values()
            .flatten()
            .filter(|g| g.status == super::generation::GenerationStatus::Probing)
            .map(|g| g.temp_key.clone())
            .collect();

        // First pass: check processes and collect health check configs
        for (process_id, process) in self.processes.iter_mut() {
            if probing_keys.contains(process_id) {
                continue;
            }

            // Check if process is still running
            if !process.is_running() {
                // A nonzero cgroup oom_kill counter means the kernel OOM
                // killer (not a normal crash) ended this process: surface
                // that distinction in stats rather than reporting Crashed.
                match process.oom_kill_count() {
                    Some(count) if count > 0 => {
                        tracing::warn!(
                            "Process {} was OOM-killed by the kernel (oom_kill={})",
                            process_id,
                            count
                        );
                        self.stats.mark_oom_killed(process_id);
                    }
                    _ => {
                        tracing::warn!("Process {} has crashed", process_id);
                        self.stats.mark_crashed(process_id);
                    }
                }

                // Enforce max_restarts: stop trying once the limit is hit.
                let max_restarts = process.config.options.max_restarts;
                if process.restart_count >= max_restarts {
                    tracing::error!(
                        "Process {} has crashed {} time(s) (max_restarts={}), giving up",
                        process_id,
                        process.restart_count,
                        max_restarts
                    );
                    // Mark as failed in stats and queue for removal so the dead child
                    // entry is dropped (reaping the zombie) and stops polluting logs.
                    self.stats.mark_crashed(process_id);
                    to_restart.push(format!("__remove__{}", process_id));
                    continue;
                }

                // Calculate backoff time based on restart count with jitter
                // Jitter prevents thundering herd when many processes crash simultaneously
                let base_backoff =
                    std::cmp::min(60, 2_i32.pow(process.restart_count.min(6))) as u64;
                let jitter = (process.pid_as_u32() % 10) as u64; // 0-9 second jitter based on PID
                let backoff = base_backoff + jitter;

                // Only restart if enough time has passed since the last restart
                if process.last_restart.elapsed().as_secs() >= backoff {
                    to_restart.push(process_id.to_string());
                }
                continue;
            }

            // Process is running - update stats
            self.stats.mark_running(process_id, process.pid_as_u32());

            // Update resource usage
            if let Some((cpu_ms, memory)) = get_process_resources(process.pid_as_u32()) {
                self.stats.update_resource_usage(process_id, cpu_ms, memory);
            }

            // Collect health check configs for later
            if let Some(health_config) = &process.health_check_config {
                health_checks.push((process_id.to_string(), health_config.clone()));
            }
        }

        // Second pass: perform health checks (avoids borrow checker issues)
        for (process_id, health_config) in health_checks {
            let health_status = self.perform_health_check(&process_id, &health_config);

            // Update stats with health check result
            self.stats
                .update_health_check(&process_id, health_status.clone());

            // Update consecutive failures
            if let Some(process) = self.processes.get_mut(&process_id) {
                match health_status {
                    HealthStatus::Healthy => {
                        process.consecutive_health_failures = 0;
                    }
                    _ => {
                        tracing::warn!(
                            "Health check for {} failed: {:?}",
                            process_id,
                            health_status
                        );
                        process.consecutive_health_failures += 1;

                        // Restart if too many consecutive failures
                        if process.consecutive_health_failures >= health_config.retries {
                            tracing::warn!(
                                "Process {} failed {} consecutive health checks, restarting",
                                process_id,
                                process.consecutive_health_failures
                            );
                            to_restart.push(process_id);
                        }
                    }
                }
            }
        }

        // Restart processes that need it; entries prefixed "__remove__" are
        // permanently failed processes that must be removed without restarting.
        // Limit concurrent restarts to prevent thundering herd
        const MAX_RESTARTS_PER_CYCLE: usize = 5;
        let mut restarts_this_cycle = 0;

        for process_id in to_restart {
            if let Some(id) = process_id.strip_prefix("__remove__") {
                // Remove the dead entry so Drop reaps the zombie child.
                self.processes.remove(id);
                tracing::error!(
                    "Process {} permanently failed; removed from supervision",
                    id
                );
            } else {
                // Stagger restarts to prevent system overload
                if restarts_this_cycle < MAX_RESTARTS_PER_CYCLE {
                    self.restart_process(&process_id)?;
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

    /// Perform an HTTP health check on a process.
    pub(super) fn perform_health_check(
        &self,
        process_id: &str,
        config: &HealthCheck,
    ) -> HealthStatus {
        use reqwest::blocking::Client;

        let port = self.get_process_port(process_id);
        let url = match port {
            Some(p) => format!("http://127.0.0.1:{}{}", p, config.url),
            None => return HealthStatus::Error("No port configured".to_string()),
        };

        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout))
            .build()
            .unwrap_or_else(|_| Client::new());

        match client.get(&url).send() {
            Ok(response) => {
                if response.status().is_success() {
                    HealthStatus::Healthy
                } else {
                    HealthStatus::Unhealthy
                }
            }
            Err(e) => {
                if e.is_timeout() {
                    HealthStatus::Timeout
                } else {
                    HealthStatus::Error(e.to_string())
                }
            }
        }
    }

    /// Get the port for a process from its environment.
    pub(super) fn get_process_port(&self, process_id: &str) -> Option<u16> {
        self.processes
            .get(process_id)
            .and_then(|p| p.config.env.get("PORT"))
            .and_then(|p: &String| p.parse::<u16>().ok())
    }

    /// Restart a specific process.
    pub(super) fn restart_process(&mut self, process_id: &str) -> Result<()> {
        tracing::info!("Restarting process: {}", process_id);

        // Capture config and current restart count before removing the process.
        let (config, prev_restart_count) = match self.processes.get(process_id) {
            Some(p) => (Some(p.config.clone()), p.restart_count),
            None => (None, 0),
        };

        if let Some(config) = config {
            // Mark as restarting in stats
            self.stats.mark_restarting(process_id);

            // Remove the old process entry (terminates it via Drop).
            self.processes.remove(process_id);

            // Spawn a fresh process and then update its restart counter so that
            // the exponential backoff keeps growing across successive crashes.
            self.spawn_process(&config)?;

            if let Some(new_process) = self.processes.get_mut(process_id) {
                new_process.restart_count = prev_restart_count + 1;
                new_process.last_restart = std::time::Instant::now();
            }
        }

        Ok(())
    }
}
