//! Taking one app's legacy workers out of the supervisor during a migration
//! and putting them back when it fails.
//!
//! A worker is parked by renaming its `workers_enabled` link to
//! `<name>.toml.migrating`: the supervisor only loads `.toml` files, so a
//! reload stops it, and renaming it back brings the old stack up again. A
//! parked link left by an interrupted migration is picked up again here.

use std::collections::BTreeSet;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};

use super::legacy::{enabled_entries, is_legacy_worker, read_worker};
use crate::config::RikuPaths;
use crate::supervisor_ctl::{notify_supervisor_reload, pid_is_alive};

const POLL_INTERVAL: Duration = Duration::from_millis(200);
const STOP_EXTRA: Duration = Duration::from_secs(30);
const ENABLED: &str = ".toml";
const PARKED: &str = ".toml.migrating";

/// Poll until none of `pids` is alive, or fail once `limit` has passed.
pub(crate) fn wait_pids_stopped(app: &str, pids: &[i32], limit: Duration) -> Result<()> {
    let start = Instant::now();
    loop {
        if !pids.iter().any(|&pid| pid_is_alive(pid)) {
            return Ok(());
        }
        if start.elapsed() >= limit {
            return Err(anyhow!(
                "the old workers of '{app}' did not stop within {} s",
                limit.as_secs()
            ));
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}

fn rename(paths: &RikuPaths, name: &str, from: &str, to: &str) -> Result<()> {
    let source = paths.workers_enabled.join(format!("{name}{from}"));
    let target = paths.workers_enabled.join(format!("{name}{to}"));
    match fs::rename(&source, &target) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
        Err(e) => Err(anyhow!("cannot rename {}: {e}", source.display())),
    }
}

fn remove_if_present(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
        Err(e) => Err(anyhow!("cannot remove {}: {e}", path.display())),
    }
}

/// The legacy workers of one app, by process id (`<app>-<kind>-<ordinal>`).
#[derive(Debug)]
pub(crate) struct LegacyWorkers {
    app: String,
    names: Vec<String>,
    longest_grace: Duration,
}

impl LegacyWorkers {
    /// Find the app's enabled or already parked legacy workers.
    pub(crate) fn load(app: &str, paths: &RikuPaths) -> Result<LegacyWorkers> {
        let entries = match enabled_entries(paths) {
            Ok(entries) => entries,
            Err(e) => return Err(e),
        };
        let prefix = format!("{app}-");
        let mut workers = LegacyWorkers {
            app: app.to_string(),
            names: Vec::new(),
            longest_grace: Duration::ZERO,
        };
        for path in entries {
            let file_name = path.file_name().map(|n| n.to_string_lossy().into_owned());
            let name = match file_name.as_deref() {
                Some(file) if file.starts_with(&prefix) => file
                    .strip_suffix(ENABLED)
                    .or_else(|| file.strip_suffix(PARKED)),
                _ => None,
            };
            let Some(name) = name else {
                continue;
            };
            let config = match read_worker(&path) {
                Ok(config) => config,
                Err(e) => return Err(e),
            };
            if config.worker.app == app && is_legacy_worker(&config) {
                let grace = Duration::from_secs(config.options.grace_period);
                workers.longest_grace = workers.longest_grace.max(grace);
                workers.names.push(name.to_string());
            }
        }
        Ok(workers)
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.names.is_empty()
    }

    /// Rename every worker link to its parked name and have the supervisor
    /// stop them.
    pub(crate) fn park(&self, paths: &RikuPaths) -> Result<()> {
        for name in &self.names {
            if let Err(e) = rename(paths, name, ENABLED, PARKED) {
                return Err(e);
            }
        }
        notify_supervisor_reload(paths);
        Ok(())
    }

    /// Rename every parked link back and have the supervisor start them.
    pub(crate) fn restore(&self, paths: &RikuPaths) -> Result<()> {
        for name in &self.names {
            if let Err(e) = rename(paths, name, PARKED, ENABLED) {
                return Err(e);
            }
        }
        notify_supervisor_reload(paths);
        Ok(())
    }

    /// Delete the parked links and their configs in `workers_available`.
    pub(crate) fn discard(&self, paths: &RikuPaths) -> Result<()> {
        for name in &self.names {
            let parked = paths.workers_enabled.join(format!("{name}{PARKED}"));
            let available = paths.workers_available.join(format!("{name}{ENABLED}"));
            for path in [parked, available] {
                if let Err(e) = remove_if_present(&path) {
                    return Err(e);
                }
            }
        }
        Ok(())
    }

    /// Wait for the processes the supervisor last reported for these
    /// workers to exit, up to the longest grace period plus 30 s. Without a
    /// stats file there is nothing to wait on.
    pub(crate) fn wait_stopped(&self, paths: &RikuPaths) -> Result<()> {
        let stats_file = paths.riku_root.join("stats.json");
        let Some(apps) = crate::supervisor::stats::read_stats_file(&stats_file) else {
            return Ok(());
        };
        let names: BTreeSet<&str> = self.names.iter().map(String::as_str).collect();
        let pids: Vec<i32> = apps
            .into_iter()
            .filter(|stats| stats.app == self.app)
            .flat_map(|stats| stats.processes)
            .filter(|process| names.contains(process.process_id.as_str()))
            .filter_map(|process| process.pid)
            .filter_map(|pid| i32::try_from(pid).ok())
            .collect();
        wait_pids_stopped(&self.app, &pids, self.longest_grace + STOP_EXTRA)
    }
}
