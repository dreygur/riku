//! Background dispatch of container restarts.
//!
//! Restarts block through the health check wait, so each restart runs on its
//! own background thread. In-flight tracking prevents duplicate restarts for
//! the same container name while a restart is already running.

use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use riku_config::RikuPaths;
use riku_container::engine::{ContainerId, ContainerInfo};

use crate::health::actions::{RestartOutcome, SharedActions};

/// Result of a completed container restart attempt.
pub(crate) struct RestartReport {
    pub(crate) name: String,
    /// The dead container the restart was asked for.
    pub(crate) id: ContainerId,
    pub(crate) outcome: Result<RestartOutcome>,
}

/// Tracks and dispatches container restarts on background threads.
pub(crate) struct Restarts {
    tx: mpsc::Sender<RestartReport>,
    rx: mpsc::Receiver<RestartReport>,
    /// The dead container id each running restart was asked for, by name.
    in_flight: HashMap<String, ContainerId>,
}

impl Restarts {
    pub(crate) fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            tx,
            rx,
            in_flight: HashMap::new(),
        }
    }

    pub(crate) fn is_in_flight(&self, name: &str) -> bool {
        self.in_flight.contains_key(name)
    }

    /// Whether a running restart of `name` already put `id` in place of the
    /// dead copy. Its report, and so the tally, only catches up on a later
    /// tick.
    pub(crate) fn replaced(&self, name: &str, id: &ContainerId) -> bool {
        self.in_flight.get(name).is_some_and(|dead| dead != id)
    }

    pub(crate) fn names(&self) -> impl Iterator<Item = String> + '_ {
        self.in_flight.keys().cloned()
    }

    pub(crate) fn collect(&mut self) -> Vec<RestartReport> {
        let reports: Vec<RestartReport> = self.rx.try_iter().collect();
        for report in &reports {
            self.in_flight.remove(&report.name);
        }
        reports
    }

    pub(crate) fn dispatch(
        &mut self,
        paths: &RikuPaths,
        actions: &SharedActions,
        info: &ContainerInfo,
    ) -> Result<()> {
        let app = match &info.app {
            Some(a) => a.clone(),
            None => return Err(anyhow!("container has no app label")),
        };
        let service = match &info.service {
            Some(s) => s.as_str().to_string(),
            None => return Err(anyhow!("container has no service label")),
        };
        let copy = match &info.copy {
            Some(c) => c.get(),
            None => return Err(anyhow!("container has no copy label")),
        };

        let name = info.name.clone();
        let id = info.id.clone();
        let paths = paths.clone();
        let actions = Arc::clone(actions);
        let tx = self.tx.clone();

        self.in_flight.insert(name.clone(), id.clone());

        let thread_name = format!("restart-{name}");
        let spawn_res = std::thread::Builder::new()
            .name(thread_name)
            .spawn(move || {
                let outcome = actions.restart_copy(&paths, &app, &service, copy);
                let _ = tx.send(RestartReport { name, id, outcome });
            });

        match spawn_res {
            Ok(_) => Ok(()),
            Err(e) => {
                self.in_flight.remove(&info.name);
                Err(anyhow!("cannot spawn restart thread: {e}"))
            }
        }
    }
}
