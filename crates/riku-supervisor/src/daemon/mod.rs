//! Supervisor daemon — owns the `Supervisor` struct and its main event loop.
//!
//! Monitors `workers-enabled/` for TOML config changes, spawns/restarts processes,
//! drives log rotation, cron scheduling, and the periodic stats writer.

pub mod config_watcher;
pub mod cron_tasks;
pub mod init;
pub mod maintenance;

use anyhow::Result;
use notify::{RecursiveMode, Watcher};
use std::collections::HashMap;
use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;
use threadpool::ThreadPool;
use tokio::sync::broadcast;

use super::{is_running, setup_signal_handlers, RELOAD_COUNTER};
use crate::cron::CronScheduler;
use crate::log_rotation::LogRotator;
use crate::process::ProcessManager;

/// Whether the supervisor should treat startup diagnostics as production
/// incidents (escalate to `error` + stderr) rather than dev-environment
/// noise (`warn` only).
///
/// Defaults to production: riku's only real deployment target is a
/// long-running PaaS host, so the safer default is to surface
/// infrastructure problems loudly. Set `RIKU_ENV=development` (or `dev`)
/// when running the supervisor locally against a sandbox without cgroup v2
/// delegated, where this check is expected to fail.
fn is_production_mode() -> bool {
    !matches!(
        std::env::var("RIKU_ENV").as_deref(),
        Ok("development") | Ok("dev")
    )
}

/// Main supervisor daemon that monitors worker configurations and manages processes.
pub struct Supervisor {
    pub(super) config_dir: std::path::PathBuf,
    pub(super) process_manager: ProcessManager,
    pub(super) watched_configs: HashMap<String, std::time::SystemTime>,
    pub(super) log_rotator: LogRotator,
    pub(super) log_root: std::path::PathBuf,
    pub(super) last_log_rotation: std::time::SystemTime,
    pub(super) log_rotation_interval: Duration,
    pub(super) stats_file: std::path::PathBuf,
    pub(super) pid_file: std::path::PathBuf,
    pub(super) control_token_file: std::path::PathBuf,
    pub(super) last_stats_write: std::time::SystemTime,
    pub(super) stats_write_interval: Duration,
    pub(super) cron_scheduler: CronScheduler,
    pub(super) last_cron_check: std::time::SystemTime,
    pub(super) cron_check_interval: Duration,
    pub(super) start_time: std::time::SystemTime,
    pub(super) health_running: Arc<AtomicBool>,
    pub(super) cron_thread_pool: ThreadPool,
    pub(super) pid_file_lock: Option<fs::File>,
    /// Broadcast sender for pushing pre-serialized metrics JSON to SSE clients.
    /// `None` if the health server failed to start.
    pub(super) metrics_broadcast_tx: Option<broadcast::Sender<String>>,
    /// Control-plane action implementation injected by the binary; defaults to
    /// a no-op so the supervisor crate stays independent of `cli`/`deploy`.
    pub(super) actions: crate::health::SharedActions,
}

impl Supervisor {
    /// Start the supervisor daemon loop.
    pub fn run(&mut self) -> Result<()> {
        tracing::info!("Starting riku supervisor daemon...");
        tracing::info!("Monitoring: {}", self.config_dir.display());
        tracing::info!("Press Ctrl+C to stop");

        // Create PID file with exclusive lock to prevent multiple supervisors
        let my_pid = std::process::id();
        match self.create_pid_file_with_lock(my_pid) {
            Ok(file) => {
                self.pid_file_lock = Some(file);
            }
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "Another supervisor is already running (PID file locked): {}",
                    e
                ));
            }
        }

        // Set up signal handlers for graceful shutdown
        setup_signal_handlers()?;

        // Async, non-blocking SIGHUP listener (config hot-reload trigger).
        // Runs on its own dedicated thread/runtime — never touches this
        // (synchronous) main loop's thread directly, just increments
        // RELOAD_COUNTER, which the loop below already polls every
        // iteration regardless of where the increment came from.
        crate::spawn_sighup_listener();

        // Best-effort check that cgroup v2 isolation, if any worker opts
        // into it, will actually work. Non-fatal: isolation is opt-in per
        // worker, so a riku deployment that never uses it should still run.
        // Without this check the first failure surfaces deep inside
        // spawn_process the moment someone enables isolation.
        if let Err(e) = crate::cgroups::verify_root_writable() {
            let diagnostic = crate::cgroups::startup_diagnostic(&e);
            if is_production_mode() {
                // Production deployments shouldn't have to go digging
                // through `RUST_LOG=debug` output to find this: escalate to
                // error level and also print straight to stderr, so it's
                // visible at boot regardless of the configured log filter
                // (the default `EnvFilter` is `info`, which would show a
                // `tracing::warn!` too, but operators frequently redirect
                // stdout/stderr to a log file and tail it directly).
                tracing::error!("{}", diagnostic);
                eprintln!("{}", diagnostic);
            } else {
                tracing::warn!("{}", diagnostic);
            }
        }

        // Start health check server
        let health_port = std::env::var("RIKU_HEALTH_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(9091);

        if let Ok(tx) = crate::health::start_health_server(
            health_port,
            self.health_running.clone(),
            self.start_time,
            self.stats_file.clone(),
            self.control_token_file.clone(),
            self.actions.clone(),
        ) {
            self.metrics_broadcast_tx = Some(tx);
        } else {
            tracing::warn!("Failed to start health server on port {}", health_port);
        }

        // Load existing configurations at startup
        self.load_initial_configs()?;

        let initial_count = self.process_manager.get_process_count();
        tracing::info!("Loaded {} worker configurations", initial_count);

        // Set up file watcher for config directory with symlink following enabled
        let (tx, rx) = mpsc::channel();
        let mut watcher = notify::RecommendedWatcher::new(
            tx,
            notify::Config::default().with_follow_symlinks(true),
        )?;
        watcher.watch(&self.config_dir, RecursiveMode::NonRecursive)?;

        tracing::info!("Supervisor running. Waiting for configuration changes...");

        // Main event loop
        loop {
            // Check if we should shut down (SIGTERM/SIGINT received)
            if !is_running() {
                tracing::info!("Received shutdown signal. Shutting down supervisor...");
                break;
            }

            // Check if reload was requested via SIGHUP
            // Use swap to atomically get and reset the counter, preventing signal loss
            let pending_reloads = RELOAD_COUNTER.swap(0, Ordering::SeqCst);
            if pending_reloads > 0 {
                tracing::info!(
                    "Received {} reload request(s). Reloading all configurations...",
                    pending_reloads
                );
                // reload_all_configs() diffs current worker TOML manifests
                // against `watched_configs` (riku's live process tree) and
                // only touches what's new, modified, or removed —
                // unchanged workers are never stopped or restarted.
                self.reload_all_configs()?;

                // Refresh nginx's routing config too, so a SIGHUP-triggered
                // reload reconciles both halves of "live config" together.
                // `nginx -s reload` is itself graceful (finishes in-flight
                // connections on old workers), so this never drops traffic
                // for unaffected apps either.
                crate::nginx::reload_nginx();
            }

            match rx.recv_timeout(Duration::from_secs(1)) {
                Ok(event) => {
                    self.handle_file_event(event?)?;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    // Periodic maintenance tasks - check process health
                    self.process_manager.check_processes()?;

                    // Drain canary probe outcomes: promote healthy generations,
                    // roll back failed ones. Never touches the stable generation
                    // unless promotion succeeds.
                    if let Err(e) = self.process_manager.reconcile_generations() {
                        tracing::error!("Generation reconciliation error: {:?}", e);
                    }

                    // Forward any rollback/promotion notifications onto the same
                    // broadcast channel the metrics SSE stream uses. `send` is
                    // non-blocking for the same reason the stats frame below is.
                    if let Some(tx) = &self.metrics_broadcast_tx {
                        for event in self.process_manager.drain_deployment_events() {
                            let _ = tx.send(event);
                        }
                    }

                    // Check if it's time for log rotation
                    if self
                        .last_log_rotation
                        .elapsed()
                        .unwrap_or(Duration::from_secs(0))
                        >= self.log_rotation_interval
                    {
                        if let Err(e) = self.rotate_logs() {
                            tracing::error!("Log rotation error: {:?}", e);
                        }
                        self.last_log_rotation = std::time::SystemTime::now();
                    }

                    // Check if it's time to write stats
                    if self
                        .last_stats_write
                        .elapsed()
                        .unwrap_or(Duration::from_secs(0))
                        >= self.stats_write_interval
                    {
                        if let Err(e) = self.write_stats() {
                            tracing::error!("Failed to write stats: {:?}", e);
                        }

                        if let Some(tx) = &self.metrics_broadcast_tx {
                            let json = serde_json::to_string(
                                &self.process_manager.stats().get_all_stats(),
                            )
                            .unwrap_or_default();
                            // `broadcast::Sender::send` never blocks the supervisor hot
                            // loop: with no subscribers it just errors (ignored here),
                            // and a full ring buffer overwrites the oldest frame instead
                            // of waiting on a slow SSE client.
                            let _ = tx.send(json);
                        }

                        self.last_stats_write = std::time::SystemTime::now();
                    }

                    // Check if it's time to check cron jobs
                    if self
                        .last_cron_check
                        .elapsed()
                        .unwrap_or(Duration::from_secs(0))
                        >= self.cron_check_interval
                    {
                        if let Err(e) = self.check_cron_jobs() {
                            tracing::error!("Cron job check error: {:?}", e);
                        }
                        self.last_cron_check = std::time::SystemTime::now();
                    }
                }
                Err(e) => {
                    tracing::error!("Watcher error: {:?}", e);
                }
            }
        }

        // Clean shutdown
        tracing::info!("Shutting down health server...");
        self.health_running.store(false, Ordering::SeqCst);

        tracing::info!("Waiting for cron jobs to complete...");
        self.cron_thread_pool.join();

        tracing::info!("Stopping all managed processes...");
        self.process_manager.stop_all_processes()?;

        // Drop PID file lock (releases exclusive lock automatically)
        drop(self.pid_file_lock.take());

        // Remove PID file on clean exit
        let _ = fs::remove_file(&self.pid_file);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::create_worker_config;
    use std::collections::HashMap;
    use tempfile::TempDir;

    fn write_sleep_worker_config(config_dir: &std::path::Path, log_dir: &std::path::Path) {
        let config = create_worker_config(
            "sighuptest",
            "web",
            "sleep 60",
            1,
            HashMap::new(),
            "/tmp",
            log_dir.join("web.1.log").to_str().unwrap(),
        );
        let toml_str = toml::to_string(&config).unwrap();
        std::fs::write(config_dir.join("sighuptest-web-1.toml"), toml_str).unwrap();
    }

    /// End-to-end regression test for the SIGHUP hot-reload path: fires a
    /// *real* `SIGHUP` at this test process via `nix::sys::signal::kill`
    /// (not a direct function call), proving the async
    /// `tokio::signal::unix` listener spawned by `spawn_sighup_listener`
    /// actually catches process-level signal delivery — not just that the
    /// reload logic works when called directly.
    ///
    /// Also proves the reload is non-destructive: a worker whose config
    /// file didn't change keeps the exact same PID across the reload, i.e.
    /// `reload_all_configs`'s mtime diff against `watched_configs` (the
    /// live process tree) correctly skips it rather than restarting
    /// everything on every SIGHUP.
    #[test]
    fn test_sighup_triggers_reload_without_disturbing_unchanged_worker() {
        let tmp = TempDir::new().unwrap();
        let riku_root = tmp.path().join(".riku");
        let config_dir = riku_root.join("workers-enabled");
        let log_dir = riku_root.join("logs");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::create_dir_all(&log_dir).unwrap();

        write_sleep_worker_config(&config_dir, &log_dir);

        let mut supervisor = Supervisor::new(config_dir.clone()).unwrap();
        supervisor.load_initial_configs().unwrap();
        assert_eq!(
            supervisor.process_manager.get_process_count(),
            1,
            "the sleep worker should be spawned by load_initial_configs"
        );

        let pid_before = supervisor
            .process_manager
            .list_processes()
            .into_iter()
            .find(|p| p.process_id == "sighuptest-web-1")
            .expect("worker should be registered before reload")
            .pid;

        // Start the real async listener under test, then fire an actual
        // SIGHUP at this process — exercising real kernel signal delivery
        // end to end, not a synthetic counter bump.
        crate::spawn_sighup_listener();
        RELOAD_COUNTER.store(0, Ordering::SeqCst);

        nix::sys::signal::kill(nix::unistd::Pid::this(), nix::sys::signal::Signal::SIGHUP)
            .expect("failed to send SIGHUP to self");

        // The listener runs on its own thread/runtime asynchronously, so
        // poll briefly rather than assuming instant delivery.
        let mut caught = false;
        for _ in 0..50 {
            if RELOAD_COUNTER.load(Ordering::SeqCst) > 0 {
                caught = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(
            caught,
            "tokio::signal::unix SIGHUP listener did not observe the signal within 1s"
        );

        // Mirror exactly what the main loop does on a pending reload.
        RELOAD_COUNTER.store(0, Ordering::SeqCst);
        supervisor.reload_all_configs().unwrap();

        assert_eq!(
            supervisor.process_manager.get_process_count(),
            1,
            "reload must not have added or removed the worker"
        );
        let pid_after = supervisor
            .process_manager
            .list_processes()
            .into_iter()
            .find(|p| p.process_id == "sighuptest-web-1")
            .expect("worker should still be registered after reload")
            .pid;
        assert_eq!(
            pid_before, pid_after,
            "an unchanged worker config must not be restarted by a SIGHUP reload \
             (same PID before and after)"
        );

        supervisor.process_manager.stop_all_processes().unwrap();
    }
}
