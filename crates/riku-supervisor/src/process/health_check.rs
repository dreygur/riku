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
        let paths = riku_config::RikuPaths::from_env();
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

#[cfg(test)]
mod tests {
    use super::ProcessManager;
    use crate::config::{WorkerConfig, WorkerInfo, WorkerOptions};
    use std::collections::HashMap;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::path::Path;
    use std::sync::Mutex;
    use tempfile::TempDir;

    // `emit_app_restarted` reads `RIKU_ROOT` via `RikuPaths::from_env()`, and
    // the notifier plugin reads `RIKU_NOTIFY_WEBHOOK_URL` — both are process
    // env vars. Serialize tests in this module that touch them so they don't
    // race each other (no other test in this crate reads these names).
    static ENV_GUARD: Mutex<()> = Mutex::new(());

    fn minimal_config(command: &str, working_dir: &str, log_file: &str) -> WorkerConfig {
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

    /// A real crash, detected through the actual `check_processes()` entry
    /// point (not a hand-rolled shortcut), must reach the real
    /// `plugins/riku-notify` bundle and deliver a webhook with the crashed
    /// process's real exit code, instance id, and restart count.
    #[test]
    fn crash_triggers_app_restarted_event_through_the_real_notify_plugin() {
        let _guard = ENV_GUARD.lock().unwrap();

        let tmp = TempDir::new().unwrap();
        let riku_root = tmp.path().join(".riku");

        // Install the real bundle exactly as `riku install-plugins --only
        // riku-notify` would, not a test fixture standing in for it.
        // CARGO_MANIFEST_DIR = .../riku/crates/riku-supervisor
        let repo_bundle = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins/riku-notify");
        let dest = riku_root.join("plugins").join("riku-notify");
        std::fs::create_dir_all(dest.join("bin")).unwrap();
        std::fs::copy(
            repo_bundle.join("riku-plugin.toml"),
            dest.join("riku-plugin.toml"),
        )
        .unwrap();
        std::fs::copy(repo_bundle.join("bin/on-event"), dest.join("bin/on-event")).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(
                dest.join("bin/on-event"),
                std::fs::Permissions::from_mode(0o755),
            )
            .unwrap();
        }

        // Non-blocking with its own deadline: if the plugin never fires (a
        // real regression, or the env-var-timing race this test used to
        // have), this must fail loudly rather than hang the whole suite.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let port = listener.local_addr().unwrap().port();
        let received = std::thread::spawn(move || {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
            loop {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let _ = stream
                            .set_read_timeout(Some(std::time::Duration::from_secs(5)));
                        let mut buf = [0u8; 4096];
                        let n = stream.read(&mut buf).unwrap_or(0);
                        let _ = stream.write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 0\r\n\r\n");
                        return Some(String::from_utf8_lossy(&buf[..n]).to_string());
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        if std::time::Instant::now() > deadline {
                            return None;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                    Err(_) => return None,
                }
            }
        });

        std::env::set_var("RIKU_ROOT", &riku_root);
        std::env::set_var(
            "RIKU_NOTIFY_WEBHOOK_URL",
            format!("http://127.0.0.1:{port}/incident"),
        );

        let log_path = tmp.path().join("test.log");
        let config = minimal_config(
            "sh -c 'exit 42'",
            tmp.path().to_str().unwrap(),
            log_path.to_str().unwrap(),
        );

        let mut pm = ProcessManager::new().expect("ProcessManager::new should succeed");
        pm.spawn_process(&config).expect("spawn should succeed");

        // Poll the real production entry point until the crash is detected
        // and restarted, rather than reaching into internals to force it.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        loop {
            pm.check_processes().expect("check_processes should not error");
            let restarted = pm
                .processes
                .get("testapp-web-1")
                .map(|p| p.restart_count >= 1)
                .unwrap_or(false);
            if restarted {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "crash was never detected and restarted"
            );
            std::thread::sleep(std::time::Duration::from_millis(200));
        }

        // Do NOT clear the env vars yet: `emit_app_restarted` (called from
        // inside `restart_process()`, above) spawns its own background
        // thread that reads them independently of this thread's timing —
        // clearing them here raced that thread and made this test hang
        // (it fell back to `$HOME/.riku` with no webhook URL configured,
        // so the plugin never fired and `received.join()` blocked forever).
        // Keep them live until the webhook has actually been observed.
        let request = received
            .join()
            .expect("webhook listener thread should not panic")
            .expect("plugin never delivered the webhook within the deadline");

        std::env::remove_var("RIKU_NOTIFY_WEBHOOK_URL");
        std::env::remove_var("RIKU_ROOT");

        assert!(request.contains("POST /incident"), "got: {request}");
        assert!(request.contains("\"app\":\"testapp\""), "got: {request}");
        assert!(
            request.contains("\"instance\":\"testapp-web-1\""),
            "got: {request}"
        );
        assert!(request.contains("\"exit_code\":42"), "got: {request}");
        assert!(request.contains("\"restart_count\":1"), "got: {request}");
    }
}
