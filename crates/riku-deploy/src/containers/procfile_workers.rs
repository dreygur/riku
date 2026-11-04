//! Taking the Procfile workers of an app that switched to containers out of
//! the supervisor.
//!
//! A worker is parked by renaming its `workers_enabled` link to
//! `<name>.toml.migrating`: the supervisor only loads `.toml` files, so a
//! reload stops it. A parked link left by an interrupted retire is picked up
//! again here.

use std::collections::BTreeSet;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::time::{Duration, Instant, SystemTime};

use anyhow::{anyhow, Result};
use riku_util::file_names::worker::WorkerName;

use super::worker_files::{enabled_entries, read_worker};
use crate::config::RikuPaths;
use crate::supervisor::stats::{read_stats_file, ProcessStats, ProcessStatus};
use crate::supervisor_ctl::{notify_supervisor_reload, pid_is_alive, supervisor_is_running};

const POLL_INTERVAL: Duration = Duration::from_millis(200);
const STOP_EXTRA: Duration = Duration::from_secs(30);
const ENABLED: &str = ".toml";
const PARKED: &str = ".toml.migrating";

fn snapshot_is_fresh(stats_file: &Path, parked_at: SystemTime) -> bool {
    match fs::metadata(stats_file).and_then(|meta| meta.modified()) {
        Ok(modified) => modified >= parked_at,
        Err(_) => false,
    }
}

fn is_running(process: &ProcessStats) -> bool {
    let alive = process
        .pid
        .and_then(|pid| i32::try_from(pid).ok())
        .is_some_and(pid_is_alive);
    process.status == ProcessStatus::Running || alive
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

/// The worker an enabled or parked entry is named after.
fn worker_id(path: &Path) -> Option<WorkerName> {
    let file_name = path.file_name().and_then(|name| name.to_str());
    file_name
        .and_then(|file| {
            file.strip_suffix(ENABLED)
                .or_else(|| file.strip_suffix(PARKED))
        })
        .and_then(WorkerName::parse)
}

fn remove_if_present(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
        Err(e) => Err(anyhow!("cannot remove {}: {e}", path.display())),
    }
}

/// The Procfile workers of one app, by process id (`<app>@<kind>.<ordinal>`).
#[derive(Debug)]
pub(crate) struct ProcfileWorkers {
    app: String,
    names: Vec<String>,
    longest_grace: Duration,
}

impl ProcfileWorkers {
    /// Find every enabled or parked worker of the app.
    pub(crate) fn load(app: &str, paths: &RikuPaths) -> Result<ProcfileWorkers> {
        let entries = match enabled_entries(paths) {
            Ok(entries) => entries,
            Err(e) => return Err(e),
        };
        let mut workers = ProcfileWorkers {
            app: app.to_string(),
            names: Vec::new(),
            longest_grace: Duration::ZERO,
        };
        for path in entries {
            let Some(name) = worker_id(&path).filter(|id| id.app == app) else {
                continue;
            };
            let config = match read_worker(&path) {
                Ok(config) => config,
                Err(e) => return Err(e),
            };
            if config.worker.app == app {
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

    fn any_running(&self, stats_file: &Path) -> bool {
        let Some(apps) = read_stats_file(stats_file) else {
            return true;
        };
        let names: BTreeSet<&str> = self.names.iter().map(String::as_str).collect();
        apps.iter()
            .filter(|stats| stats.app == self.app)
            .flat_map(|stats| stats.processes.iter())
            .filter(|process| names.contains(process.process_id.as_str()))
            .any(is_running)
    }

    /// Wait until a supervisor snapshot taken after `parked_at` shows none of
    /// these workers running, or fail once `limit` has passed. The supervisor
    /// reloads before it writes the next snapshot, so a fresh snapshot proves
    /// the workers it was told to stop are stopped. Without a running
    /// supervisor there is nothing to wait for.
    pub(crate) fn wait_stopped_since(
        &self,
        paths: &RikuPaths,
        parked_at: SystemTime,
        limit: Duration,
    ) -> Result<()> {
        if !supervisor_is_running(paths) {
            return Ok(());
        }
        let stats_file = paths.riku_root.join("stats.json");
        let start = Instant::now();
        loop {
            if snapshot_is_fresh(&stats_file, parked_at) && !self.any_running(&stats_file) {
                return Ok(());
            }
            if start.elapsed() >= limit {
                return Err(anyhow!(
                    "the old workers of '{}' did not stop within {} s",
                    self.app,
                    limit.as_secs()
                ));
            }
            std::thread::sleep(POLL_INTERVAL);
        }
    }

    /// Wait for the old workers to stop, up to the longest grace period plus
    /// 30 s.
    pub(crate) fn wait_stopped(&self, paths: &RikuPaths, parked_at: SystemTime) -> Result<()> {
        self.wait_stopped_since(paths, parked_at, self.longest_grace + STOP_EXTRA)
    }
}

#[cfg(test)]
#[path = "procfile_wait_tests.rs"]
mod wait_tests;
