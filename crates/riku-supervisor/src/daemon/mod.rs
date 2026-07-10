//! Supervisor daemon — owns the `Supervisor` struct and its main event loop.
//!
//! Monitors `workers-enabled/` for TOML config changes, spawns/restarts processes,
//! drives log rotation, cron scheduling, and the periodic stats writer.

pub mod config_watcher;
pub mod cron_tasks;
pub mod image_watch;
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

        if let Err(e) = setup_signal_handlers() {
            return Err(e);
        }

        // Async, non-blocking SIGHUP listener (config hot-reload trigger).
        // Runs on its own dedicated thread/runtime — never touches this
        // (synchronous) main loop's thread directly, just increments
        // RELOAD_COUNTER, which the loop below already polls every
        // iteration regardless of where the increment came from.
        crate::spawn_sighup_listener();

        self.check_cgroup_isolation();
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

    /// Create the PID file with an exclusive lock, so a second supervisor
    /// can't start against the same config directory.
    fn acquire_pid_lock(&mut self) -> Result<()> {
        let my_pid = std::process::id();
        match self.create_pid_file_with_lock(my_pid) {
            Ok(file) => {
                self.pid_file_lock = Some(file);
                Ok(())
            }
            Err(e) => Err(anyhow::anyhow!(
                "Another supervisor is already running (PID file locked): {}",
                e
            )),
        }
    }

    /// Best-effort check that cgroup v2 isolation, if any worker opts into
    /// it, will actually work. Non-fatal: isolation is opt-in per worker, so
    /// a riku deployment that never uses it should still run. Without this
    /// check the first failure surfaces deep inside `spawn_process` the
    /// moment someone enables isolation.
    fn check_cgroup_isolation(&self) {
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
    }

    /// Start the health-check HTTP server, storing the metrics broadcast
    /// sender on success so the event loop can push SSE frames to it later.
    /// Non-fatal on failure: the supervisor still runs without a health
    /// endpoint, just without live metrics/control-plane access.
    fn start_health_endpoint(&mut self) {
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
    }

    /// Set up the filesystem watcher on `config_dir`. The returned watcher
    /// must be kept alive by the caller for as long as `rx` is read from —
    /// dropping it stops the watch.
    #[allow(clippy::type_complexity)]
    fn watch_config_dir(
        &self,
    ) -> Result<(
        notify::RecommendedWatcher,
        mpsc::Receiver<notify::Result<notify::Event>>,
    )> {
        let (tx, rx) = mpsc::channel();
        let mut watcher = match notify::RecommendedWatcher::new(
            tx,
            notify::Config::default().with_follow_symlinks(true),
        ) {
            Ok(w) => w,
            Err(e) => return Err(e.into()),
        };
        if let Err(e) = watcher.watch(&self.config_dir, RecursiveMode::NonRecursive) {
            return Err(e.into());
        }
        Ok((watcher, rx))
    }

    /// Main event loop: reacts to config file changes and, on each idle
    /// timeout tick, runs the periodic maintenance sweep. Runs until a
    /// shutdown signal (SIGTERM/SIGINT) is observed. A watcher-reported
    /// error, or a failure handling a config file event, aborts the loop
    /// and propagates — matching the previous inline behavior where either
    /// would exit `run()` entirely rather than being treated as recoverable.
    fn run_event_loop(&mut self, rx: &mpsc::Receiver<notify::Result<notify::Event>>) -> Result<()> {
        loop {
            // Check if we should shut down (SIGTERM/SIGINT received)
            if !is_running() {
                tracing::info!("Received shutdown signal. Shutting down supervisor...");
                break;
            }

            if let Err(e) = self.reload_if_requested() {
                return Err(e);
            }

            match rx.recv_timeout(Duration::from_secs(1)) {
                Ok(event) => {
                    let event = match event {
                        Ok(event) => event,
                        Err(e) => return Err(e.into()),
                    };
                    if let Err(e) = self.handle_file_event(event) {
                        return Err(e);
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    self.run_periodic_maintenance();
                }
                Err(e) => {
                    tracing::error!("Watcher error: {:?}", e);
                }
            }
        }

        Ok(())
    }

    /// Reload all worker configs if a SIGHUP was received since the last
    /// check. Uses `swap` to atomically get-and-reset the counter, so a
    /// signal delivered between ticks is never lost.
    fn reload_if_requested(&mut self) -> Result<()> {
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
            if let Err(e) = self.reload_all_configs() {
                return Err(e);
            }

            // Refresh nginx's routing config too, so a SIGHUP-triggered
            // reload reconciles both halves of "live config" together.
            // `nginx -s reload` is itself graceful (finishes in-flight
            // connections on old workers), so this never drops traffic
            // for unaffected apps either.
            crate::nginx::reload_nginx();
        }
        Ok(())
    }

    /// Runs on every event-loop timeout tick (i.e. no config file event in
    /// the last second): process health, canary reconciliation, log
    /// rotation, stats writing, cron, and watched-image checks, each gated
    /// on its own interval. Every failure here is logged and swallowed —
    /// none of these should ever be able to take the whole daemon down.
    fn run_periodic_maintenance(&mut self) {
        if let Err(e) = self.process_manager.check_processes() {
            tracing::error!("Process health check error: {:?}", e);
        }

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
                let json = serde_json::to_string(&self.process_manager.stats().get_all_stats())
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

        // Check if it's time to re-pull watched compose images
        if self
            .last_image_watch_check
            .elapsed()
            .unwrap_or(Duration::from_secs(0))
            >= self.image_watch_interval
        {
            if let Err(e) = self.check_watched_images() {
                tracing::error!("Watched image check error: {:?}", e);
            }
            self.last_image_watch_check = std::time::SystemTime::now();
        }
    }

    /// Clean shutdown: stop the health server, wait for in-flight cron jobs,
    /// stop every managed process, and release the PID file. Only reached
    /// when the event loop exits via a shutdown signal — an error exiting
    /// the loop skips straight to returning that error instead.
    fn shutdown(&mut self) -> Result<()> {
        tracing::info!("Shutting down health server...");
        self.health_running.store(false, Ordering::SeqCst);

        tracing::info!("Waiting for cron jobs to complete...");
        self.cron_thread_pool.join();

        tracing::info!("Stopping all managed processes...");
        if let Err(e) = self.process_manager.stop_all_processes() {
            return Err(e);
        }

        // Drop PID file lock (releases exclusive lock automatically)
        drop(self.pid_file_lock.take());

        // Remove PID file on clean exit
        let _ = fs::remove_file(&self.pid_file);

        Ok(())
    }
}

#[cfg(test)]
mod tests;
