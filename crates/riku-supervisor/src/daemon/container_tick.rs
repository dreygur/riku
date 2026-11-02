//! Periodic container watch tick driving engine event reconciliation and stats.

use std::time::{Duration, Instant, SystemTime};

use riku_config::RikuPaths;

use super::periodic::is_due;
use super::Supervisor;
use crate::containers::{ContainerWatch, CONTAINER_WATCH_INTERVAL};

const ENGINE_RETRY: Duration = Duration::from_secs(60);

impl Supervisor {
    pub(super) fn containers_if_due(&mut self) {
        if !is_due(self.last_container_watch, CONTAINER_WATCH_INTERVAL) {
            return;
        }

        if self.container_watch.is_none() {
            let retry_due = match self.last_engine_attempt {
                Some(last) => is_due(last, ENGINE_RETRY),
                None => true,
            };
            if retry_due {
                let paths = match RikuPaths::from_env() {
                    Ok(p) => p,
                    Err(e) => {
                        tracing::debug!("cannot read riku paths: {e}");
                        self.last_engine_attempt = Some(SystemTime::now());
                        self.last_container_watch = SystemTime::now();
                        return;
                    }
                };
                match ContainerWatch::open(&paths, self.actions.clone(), &self.log_root) {
                    Ok(watch) => {
                        self.container_watch = Some(watch);
                    }
                    Err(e) => {
                        tracing::debug!("container watch unavailable: {e}");
                        self.last_engine_attempt = Some(SystemTime::now());
                    }
                }
            }
        }

        if let Some(watch) = self.container_watch.as_mut() {
            watch.tick(Instant::now());
            self.process_manager
                .stats_mut()
                .replace_container_rows(watch.rows());
        }

        self.last_container_watch = SystemTime::now();
    }
}
