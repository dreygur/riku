//! Health check and process monitoring for the ProcessManager.

use anyhow::Result;
use std::cell::OnceCell;

use crate::config::HealthCheck;
use crate::stats::{HealthStatus, ProcessTable};

use super::ProcessManager;

impl ProcessManager {
    /// Check the status of all managed processes, perform health checks, and restart crashed ones.
    pub fn check_processes(&mut self) -> Result<()> {
        let (mut to_restart, health_checks) = self.scan_process_status();
        self.run_health_checks(health_checks, &mut to_restart);
        self.restart_or_remove_processes(to_restart)
    }

    /// First pass: walk every managed process, update its running/crashed
    /// status in stats, and decide which need a restart attempt or a health
    /// check. Split from `check_processes` so each pass reads as one thing.
    fn scan_process_status(&mut self) -> (Vec<String>, Vec<(String, HealthCheck)>) {
        let mut to_restart = Vec::new();
        let mut health_checks: Vec<(String, HealthCheck)> = Vec::new();

        // Generations under active probing are owned exclusively by the
        // orchestrator (`reconcile_generations` + the probe thread's circuit
        // breaker): skip them here so the two restart paths never race.
        let probing_keys: std::collections::HashSet<String> = self
            .generations
            .values()
            .flatten()
            .filter(|g| g.status == super::generation::GenerationStatus::Probing)
            .map(|g| g.temp_key.clone())
            .collect();
        let procs = OnceCell::new();

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

            let pid = process.pid_as_u32();
            let usage =
                process.resource_usage(|| procs.get_or_init(ProcessTable::read).tree_usage(pid));
            self.stats.update_resource_usage(process_id, usage);

            // Collect health check configs for later
            if let Some(health_config) = &process.health_check_config {
                health_checks.push((process_id.to_string(), health_config.clone()));
            }
        }

        (to_restart, health_checks)
    }

    /// Second pass: perform health checks (kept separate from the scan above
    /// to avoid borrow checker issues: this needs `&mut self` per process
    /// while the scan still holds an iterator over `self.processes`).
    pub(super) fn run_health_checks(
        &mut self,
        health_checks: Vec<(String, HealthCheck)>,
        to_restart: &mut Vec<String>,
    ) {
        for (process_id, health_config) in health_checks {
            if !self.claim_health_probe(&process_id, health_config.interval) {
                continue;
            }

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
    }
}

#[cfg(test)]
#[path = "health_check_tests.rs"]
mod tests;
