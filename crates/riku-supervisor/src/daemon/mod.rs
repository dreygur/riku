//! Supervisor daemon: owns the `Supervisor` struct and its main event loop.
//!
//! Monitors `workers-enabled/` for TOML config changes, spawns/restarts processes,
//! drives log rotation, cron scheduling, and the periodic stats writer.

mod config_file;
pub mod config_watcher;
pub mod container_tick;
pub mod cron_tasks;
pub mod event_loop;
pub mod image_watch;
pub mod init;
pub mod maintenance;
pub mod periodic;
pub mod startup;

use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;
use threadpool::ThreadPool;
use tokio::sync::broadcast;

use super::setup_signal_handlers;
use crate::cron::CronScheduler;
use crate::log_rotation::LogRotator;
use crate::process::ProcessManager;

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
    pub(super) last_image_watch_check: std::time::SystemTime,
    pub(super) image_watch_interval: Duration,
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
    pub(super) container_watch: Option<crate::containers::ContainerWatch>,
    pub(super) last_container_watch: std::time::SystemTime,
    pub(super) last_engine_attempt: Option<std::time::SystemTime>,
}

// Explicit match/if-let over `?` throughout this impl block per project
// convention (.claude/CLAUDE.md rule 16): every early exit stays visible as
// a `return Err(...)`.
#[allow(clippy::question_mark)]
impl Supervisor {
    /// Start the supervisor daemon loop.
    pub fn run(&mut self) -> Result<()> {
        tracing::info!("Starting riku supervisor daemon...");
        tracing::info!("Monitoring: {}", self.config_dir.display());
        tracing::info!("Press Ctrl+C to stop");

        if let Err(e) = self.acquire_pid_lock() {
            return Err(e);
        }

        // Only after the lock: a second supervisor must not wipe the
        // markers of the one that is running.
        if let Err(e) = self.process_manager.clear_stale_pid_markers() {
            return Err(anyhow::anyhow!("cannot clear stale worker pid files: {e}"));
        }

        if let Err(e) = setup_signal_handlers() {
            return Err(e);
        }

        // Async, non-blocking SIGHUP listener (config hot-reload trigger).
        // Runs on its own dedicated thread/runtime: never touches this
        // (synchronous) main loop's thread directly, just increments
        // RELOAD_COUNTER, which the loop below already polls every
        // iteration regardless of where the increment came from.
        crate::spawn_sighup_listener();

        self.check_cgroup_isolation();
        self.ensure_socket_root();
        self.start_health_endpoint();

        if let Err(e) = self.load_initial_configs() {
            return Err(e);
        }

        let initial_count = self.process_manager.get_process_count();
        tracing::info!("Loaded {} worker configurations", initial_count);

        let (_watcher, rx) = match self.watch_config_dir() {
            Ok(pair) => pair,
            Err(e) => return Err(e),
        };

        tracing::info!("Supervisor running. Waiting for configuration changes...");

        if let Err(e) = self.run_event_loop(&rx) {
            return Err(e);
        }

        self.shutdown()
    }
}

#[cfg(test)]
mod tests;
