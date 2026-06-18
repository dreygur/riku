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
use crate::supervisor::cron::CronScheduler;
use crate::supervisor::log_rotation::LogRotator;
use crate::supervisor::process::ProcessManager;

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
    #[allow(dead_code)]
    pub(super) pid_file_lock: Option<fs::File>,
    /// Broadcast sender for pushing pre-serialized metrics JSON to SSE clients.
    /// `None` if the health server failed to start.
    pub(super) metrics_broadcast_tx: Option<broadcast::Sender<String>>,
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

        // Start health check server
        let health_port = std::env::var("RIKU_HEALTH_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(9091);

        if let Ok(tx) = crate::supervisor::health::start_health_server(
            health_port,
            self.health_running.clone(),
            self.start_time,
            self.stats_file.clone(),
            self.control_token_file.clone(),
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
                self.reload_all_configs()?;
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
