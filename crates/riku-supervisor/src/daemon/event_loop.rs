//! Supervisor event loop, reload handling, and clean shutdown.

use anyhow::Result;
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
    /// Reload all worker configs if a SIGHUP was received since the last
    /// check. Uses `swap` to atomically get-and-reset the counter, so a
    /// signal delivered between ticks is never lost.
    ///
    /// The router is left alone: every command that writes router config
    /// requests and confirms its own reload. The container watch takes a
    /// listing on the next maintenance pass, since container deploys signal
    /// the same way.
    fn reload_if_requested(&mut self) -> Result<()> {
        let pending_reloads = RELOAD_COUNTER.swap(0, Ordering::SeqCst);
        if pending_reloads > 0 {
            tracing::info!(
                "Received {} reload request(s). Reloading all configurations...",
                pending_reloads
            );
            self.request_container_listing();
            // reload_all_configs() diffs current worker TOML manifests
            // against `watched_configs` (riku's live process tree) and
            // only touches what's new, modified, or removed,
            // unchanged workers are never stopped or restarted.
            if let Err(e) = self.reload_all_configs() {
                return Err(e);
            }
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
    /// stop every managed process, and release the PID file's lock. Only reached
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

        // The file stays: a starter may already have it open, and unlinking
        // it would let that one and the next start lock two different files.
        // Readers take liveness from the lock, never from the file existing.
        drop(self.pid_file_lock.take());

        Ok(())
    }
}
