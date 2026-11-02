//! Supervisor event loop, reload handling, and clean shutdown.

use anyhow::Result;
use riku_config::router::Router;
use std::fs;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::time::Duration;

use super::Supervisor;
use crate::{is_running, RELOAD_COUNTER};

// Explicit match/if-let over `?` throughout this impl block per project
// convention (.claude/CLAUDE.md rule 16): every early exit stays visible as
// a `return Err(...)`.
#[allow(clippy::question_mark)]
impl Supervisor {
    /// Refresh the router's config too, so a SIGHUP-triggered reload
    /// reconciles both halves of "live config" together. `nginx -s reload` is
    /// itself graceful (finishes in-flight connections on old workers), so
    /// this never drops traffic for unaffected apps either.
    fn reload_router(&self) {
        match Router::load_file(&self.router_file) {
            Ok(Router::Nginx) => {
                if let Err(e) = crate::nginx::reload_nginx() {
                    tracing::warn!("nginx reload failed: {e}");
                }
            }
            // Caddy is reloaded by each deploy; a router plugin reloads itself.
            Ok(Router::Caddy | Router::Plugin(_)) => {}
            Err(e) => tracing::error!("not reloading the router: {e}"),
        }
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
            // only touches what's new, modified, or removed,
            // unchanged workers are never stopped or restarted.
            if let Err(e) = self.reload_all_configs() {
                return Err(e);
            }

            self.reload_router();
        }
        Ok(())
    }

    /// Main event loop: reacts to config file changes and, on each idle
    /// timeout tick, runs the periodic maintenance sweep. Runs until a
    /// shutdown signal (SIGTERM/SIGINT) is observed. A watcher-reported
    /// error, or a failure handling a config file event, aborts the loop
    /// and propagates: matching the previous inline behavior where either
    /// would exit `run()` entirely rather than being treated as recoverable.
    pub(super) fn run_event_loop(
        &mut self,
        rx: &mpsc::Receiver<notify::Result<notify::Event>>,
    ) -> Result<()> {
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

    /// Clean shutdown: stop the health server, wait for in-flight cron jobs,
    /// stop every managed process, and release the PID file. Only reached
    /// when the event loop exits via a shutdown signal, an error exiting
    /// the loop skips straight to returning that error instead.
    pub(super) fn shutdown(&mut self) -> Result<()> {
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
