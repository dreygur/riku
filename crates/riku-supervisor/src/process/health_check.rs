//! Health check and process monitoring for the ProcessManager.

use anyhow::Result;
use std::time::Duration;

use crate::config::HealthCheck;
use crate::plugins::{EventBus, EventName};
use crate::stats::{get_process_resources, HealthStatus};

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
        // breaker) — skip them here so the two restart paths never race.
        let probing_keys: std::collections::HashSet<String> = self
            .generations
            .values()
            .flatten()
            .filter(|g| g.status == super::generation::GenerationStatus::Probing)
            .map(|g| g.temp_key.clone())
            .collect();

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

        (to_restart, health_checks)
    }

    /// Second pass: perform health checks (kept separate from the scan above
    /// to avoid borrow checker issues — this needs `&mut self` per process
    /// while the scan still holds an iterator over `self.processes`).
    fn run_health_checks(
        &mut self,
        health_checks: Vec<(String, HealthCheck)>,
        to_restart: &mut Vec<String>,
    ) {
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
    }

    /// Third pass: restart processes that need it; entries prefixed
    /// "__remove__" are permanently failed processes that must be removed
    /// without restarting. Limits concurrent restarts to prevent a
    /// thundering herd.
    // Explicit match over `?` per project convention (.claude/CLAUDE.md rule 16).
    #[allow(clippy::question_mark)]
    fn restart_or_remove_processes(&mut self, to_restart: Vec<String>) -> Result<()> {
        const MAX_RESTARTS_PER_CYCLE: usize = 5;
        let mut restarts_this_cycle = 0;

        for process_id in to_restart {
            if let Some(id) = process_id.strip_prefix("__remove__") {
                // Fire app.failed before removing — this is the one case
                // restart_process() never reaches, and it's the case an
                // admin most needs to hear about (riku has given up, not
                // just self-healed).
                if let Some(p) = self.processes.get(id) {
                    emit_app_failed(
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

            emit_app_restarted(
                config.worker.app,
                process_id.to_string(),
                exit_code,
                new_restart_count,
            );
        }

        Ok(())
    }
}

/// Fire the `app.restarted` lifecycle event (Plugin Protocol v1 §7.1) for a
/// crash we just recovered from.
///
/// Dispatched on its own thread: `EventBus::emit` runs each subscriber
/// synchronously with its own timeout, and this fires from the supervisor's
/// single-threaded 1-second monitoring tick — a slow or unreachable
/// notification target must never delay health checks for every other app.
fn emit_app_restarted(app: String, instance: String, exit_code: Option<i32>, restart_count: u32) {
    std::thread::spawn(move || {
        let paths = match riku_config::RikuPaths::from_env() {
            Ok(paths) => paths,
            Err(e) => {
                tracing::error!("Cannot emit app.restarted for {app}: {e}");
                return;
            }
        };
        EventBus::new(&paths).publish(
            EventName::AppRestarted,
            &app,
            serde_json::json!({
                "instance": instance,
                "exit_code": exit_code,
                "restart_count": restart_count,
            }),
        );
    });
}

/// Fire the `app.failed` lifecycle event for a process that crashed enough
/// times to hit `max_restarts` and is being permanently removed from
/// supervision — the case `emit_app_restarted` never covers, since
/// `restart_process()` (and therefore that emit call) is never reached once
/// riku gives up.
fn emit_app_failed(
    app: String,
    instance: String,
    exit_code: Option<i32>,
    restart_count: u32,
    max_restarts: u32,
) {
    std::thread::spawn(move || {
        let paths = match riku_config::RikuPaths::from_env() {
            Ok(paths) => paths,
            Err(e) => {
                tracing::error!("Cannot emit app.failed for {app}: {e}");
                return;
            }
        };
        EventBus::new(&paths).publish(
            EventName::AppFailed,
            &app,
            serde_json::json!({
                "instance": instance,
                "exit_code": exit_code,
                "restart_count": restart_count,
                "max_restarts": max_restarts,
            }),
        );
    });
}

#[cfg(test)]
#[path = "health_check_tests.rs"]
mod tests;
