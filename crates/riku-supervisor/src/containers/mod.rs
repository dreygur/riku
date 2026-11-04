//! Container supervision: adoption, event watching, log copying, and restarts.
//!
//! Owns the container watch loop, restart tally bookkeeping, event stream
//! ingestion, container log copying, and container statistics mapping.
//!
//! # Security Model
//!
//! Engine output and labels are untrusted input. Names and ids reach the
//! engine only as validated types (such as `ContainerId` and `ContainerName`),
//! preventing argument injection. No log or tracing statement outputs
//! environment values or raw command arguments. Logs name containers, counts,
//! validated service names and copy numbers, and the app label only through
//! `Debug`, which escapes control characters.

use std::collections::HashMap;
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::Result;
use riku_config::RikuPaths;
use riku_container::engine::{ContainerId, ContainerInfo, ContainerStats, Engine};

mod candidates;
mod feed;
mod listing;
mod log_pull;
mod log_thread;
#[cfg(test)]
mod mock_actions;
#[cfg(test)]
mod mock_engine;
mod restart_policy;
mod restarts;
mod stats;
mod tally;
#[cfg(test)]
mod test_ticks;
mod watch;

/// How often the daemon runs [`ContainerWatch::tick`].
pub const CONTAINER_WATCH_INTERVAL: Duration = Duration::from_secs(5);
/// How often the watch lists every riku container and samples engine stats.
/// Without engine events this is also how soon a crash is noticed.
pub const CONTAINER_STATS_INTERVAL: Duration = Duration::from_secs(30);

pub use restart_policy::{should_restart, RestartPolicyLabel};

use feed::EventStream;
use log_thread::LogThread;
use restarts::Restarts;
use stats::container_rows;
use tally::RestartTally;

use crate::health::actions::SharedActions;
use crate::stats::types::ProcessStats;

/// Where crash news comes from: the engine's event stream, or only the
/// listing every [`CONTAINER_STATS_INTERVAL`].
pub(crate) enum Feed {
    Events(EventStream),
    Polling,
}

/// The supervisor's view of riku containers: it adopts running copies,
/// copies their logs every tick, restarts dead ones through
/// [`crate::health::ControlActions::restart_copy`] and reports them as stats
/// rows. It never stops, kills or removes a container itself.
pub struct ContainerWatch {
    pub(crate) engine: Engine,
    pub(crate) paths: RikuPaths,
    pub(crate) actions: SharedActions,
    pub(crate) feed: Feed,
    pub(crate) known: HashMap<ContainerId, ContainerInfo>,
    pub(crate) samples: HashMap<String, ContainerStats>,
    pub(crate) tally: RestartTally,
    pub(crate) logs: LogThread,
    pub(crate) restarts: Restarts,
    pub(crate) adopted: bool,
    pub(crate) last_listing: Option<Instant>,
}

impl ContainerWatch {
    pub(crate) fn with_engine(
        engine: Engine,
        paths: RikuPaths,
        actions: SharedActions,
        log_root: &Path,
    ) -> ContainerWatch {
        ContainerWatch {
            logs: LogThread::spawn(engine.clone(), log_root.to_path_buf()),
            engine,
            paths,
            actions,
            feed: Feed::Polling,
            known: HashMap::new(),
            samples: HashMap::new(),
            tally: RestartTally::new(),
            restarts: Restarts::new(),
            adopted: false,
            last_listing: None,
        }
    }

    /// Detect and open the server's container engine with the time limits
    /// from the server settings. Fails on servers without an engine.
    pub fn open(
        paths: &RikuPaths,
        actions: SharedActions,
        log_root: &Path,
    ) -> Result<ContainerWatch> {
        let setting = match riku_config::container_engine::load(paths) {
            Ok(setting) => setting,
            Err(e) => return Err(e.context("cannot read the engine setting")),
        };
        let path_var = std::env::var_os("PATH").unwrap_or_default();
        let binary = match riku_container::engine::detect(setting.as_deref(), &path_var) {
            Ok(binary) => binary,
            Err(e) => return Err(e.context("no container engine to watch")),
        };
        let server_settings = match riku_config::settings::load(paths) {
            Ok(settings) => settings,
            Err(e) => return Err(e.context("cannot read the server settings")),
        };
        let engine = match Engine::open(binary) {
            Ok(engine) => engine.with_settings(server_settings.engine),
            Err(e) => return Err(e.context("cannot open the container engine")),
        };
        Ok(Self::with_engine(engine, paths.clone(), actions, log_root))
    }

    /// One stats row per riku container copy, keyed `<app>-<service>-<n>`.
    pub fn rows(&self) -> Vec<ProcessStats> {
        let mut infos: Vec<ContainerInfo> = self.known.values().cloned().collect();
        infos.sort_by(|a, b| a.name.cmp(&b.name));
        container_rows(&infos, &self.samples, &self.tally)
    }

    /// Run one watch step. Engine errors are logged and the step goes on;
    /// restarts run on their own threads and are collected on a later tick.
    pub fn tick(&mut self, now: Instant) {
        watch::reconcile(self, now);
    }

    /// Apply engine events that arrived since the last call, between ticks,
    /// so a replaced or removed container leaves [`ContainerWatch::rows`]
    /// without waiting for the next tick. True when the rows may have changed.
    /// Restarts and log copying still wait for [`ContainerWatch::tick`].
    pub fn apply_events(&mut self) -> bool {
        if !self.adopted {
            return false;
        }
        listing::drain_events(&self.engine, &mut self.feed, &mut self.known)
    }
}

#[cfg(test)]
#[path = "watch_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "adoption_tests.rs"]
mod adoption_tests;

#[cfg(test)]
#[path = "events_tests.rs"]
mod events_tests;
