//! Periodic maintenance tasks: process health checks, canary reconciliation,
//! and scheduled jobs.

use std::time::{Duration, SystemTime};

use super::Supervisor;

pub(super) fn is_due(last: SystemTime, interval: Duration) -> bool {
    last.elapsed().unwrap_or(Duration::from_secs(0)) >= interval
}

impl Supervisor {
    fn rotate_if_due(&mut self) {
        if !is_due(self.last_log_rotation, self.log_rotation_interval) {
            return;
        }
        if let Err(e) = self.rotate_logs() {
            tracing::error!("Log rotation error: {:?}", e);
        }
        self.last_log_rotation = SystemTime::now();
    }

    fn forget_destroyed_apps(&mut self) {
        let paths = match riku_config::RikuPaths::from_env() {
            Ok(paths) => paths,
            Err(e) => {
                tracing::debug!("cannot read riku paths: {e}");
                return;
            }
        };
        self.process_manager
            .stats_mut()
            .forget_stopped_apps(|app| paths.app_root.join(app).exists());
    }

    fn write_stats_if_due(&mut self) {
        if !is_due(self.last_stats_write, self.stats_write_interval) {
            return;
        }
        self.forget_destroyed_apps();
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

        self.last_stats_write = SystemTime::now();
    }

    fn cron_if_due(&mut self) {
        if !is_due(self.last_cron_check, self.cron_check_interval) {
            return;
        }
        if let Err(e) = self.check_cron_jobs() {
            tracing::error!("Cron job check error: {:?}", e);
        }
        self.last_cron_check = SystemTime::now();
    }

    fn images_if_due(&mut self) {
        if !is_due(self.last_image_watch_check, self.image_watch_interval) {
            return;
        }
        if let Err(e) = self.check_watched_images() {
            tracing::error!("Watched image check error: {:?}", e);
        }
        self.last_image_watch_check = SystemTime::now();
    }

    /// Runs on every event-loop timeout tick (i.e. no config file event in
    /// the last second): process health, canary reconciliation, log
    /// rotation, stats writing, cron, and watched-image checks, each gated
    /// on its own interval. Every failure here is logged and swallowed,
    /// none of these should ever be able to take the whole daemon down.
    pub(super) fn run_periodic_maintenance(&mut self) {
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

        self.rotate_if_due();
        self.write_stats_if_due();
        self.cron_if_due();
        self.images_if_due();
        self.containers_if_due();
    }
}
