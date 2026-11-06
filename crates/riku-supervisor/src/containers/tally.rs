//! Restart tally tracking per container name.

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use riku_container::engine::ContainerId;

use super::restart_policy::restart_delay;

#[derive(Debug, Clone)]
struct Tally {
    id: ContainerId,
    count: u32,
    last_restart: Option<Instant>,
    expects_successor: bool,
}

/// Tracks restart counts and backoff per container name.
#[derive(Debug, Default)]
pub(crate) struct RestartTally {
    entries: HashMap<String, Tally>,
}

impl RestartTally {
    pub(crate) fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Observe a live or exited container id for `name`.
    pub(crate) fn observe(&mut self, name: &str, id: &ContainerId) {
        match self.entries.get_mut(name) {
            None => {
                self.entries.insert(
                    name.to_string(),
                    Tally {
                        id: id.clone(),
                        count: 0,
                        last_restart: None,
                        expects_successor: false,
                    },
                );
            }
            Some(entry) => {
                if entry.id == *id {
                    return;
                }
                if !entry.expects_successor {
                    entry.count = 0;
                    entry.last_restart = None;
                }
                entry.id = id.clone();
                entry.expects_successor = false;
            }
        }
    }

    /// Number of restarts recorded for `name`.
    pub(crate) fn count(&self, name: &str) -> u32 {
        match self.entries.get(name) {
            Some(entry) => entry.count,
            None => 0,
        }
    }

    /// Whether enough time has passed since the last restart to attempt another.
    pub(crate) fn due(&self, name: &str, now: Instant) -> bool {
        let entry = match self.entries.get(name) {
            Some(e) => e,
            None => return true,
        };
        let last = match entry.last_restart {
            Some(t) => t,
            None => return true,
        };
        let delay = restart_delay(entry.count);
        match now.checked_duration_since(last) {
            Some(elapsed) => elapsed >= delay,
            None => false,
        }
    }

    /// Record that container `name` was restarted at `now`.
    pub(crate) fn record_restart(&mut self, name: &str, now: Instant) {
        if let Some(entry) = self.entries.get_mut(name) {
            entry.count += 1;
            entry.last_restart = Some(now);
            entry.expects_successor = true;
        }
    }

    /// Remove records for containers that are no longer known, except those
    /// waiting for the copy a restart started, which may not be seen yet.
    pub(crate) fn forget_missing(&mut self, live_names: &HashSet<String>) {
        self.entries
            .retain(|name, entry| entry.expects_successor || live_names.contains(name));
    }
}

#[cfg(test)]
#[path = "tally_tests.rs"]
mod tests;
