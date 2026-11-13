//! Writes to `stats.json` from more than one process. The supervisor writes
//! the whole file from memory; a container deploy replaces one app's
//! container rows right after starting its copies, so `riku apps` counts
//! them before the supervisor's next listing. Both hold an exclusive `flock`
//! on `<stats>.lock` for the write, and the deploy for its read too, so
//! neither loses the other's rows, and both replace the file by an atomic
//! rename, so a reader never sees half of one.
//!
//! Security: the lock file is created `0600` and opened `O_NOFOLLOW`, in
//! the deploy user's riku root, the only account that writes stats.

use std::fs::{self, File, OpenOptions};
use std::io::ErrorKind;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};

use super::manager::StatsManager;
use super::types::{AppStats, ProcessStats};

fn lock_path(stats: &Path) -> PathBuf {
    let mut name = stats.as_os_str().to_owned();
    name.push(".lock");
    PathBuf::from(name)
}

/// Hold the exclusive lock on `stats`'s lock file until the returned file
/// is dropped. Waits for another holder.
pub(crate) fn lock(stats: &Path) -> Result<File> {
    let path = lock_path(stats);
    let file = match OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&path)
    {
        Ok(file) => file,
        Err(e) => return Err(anyhow!("cannot open {}: {e}", path.display())),
    };
    match file.lock() {
        Ok(()) => Ok(file),
        Err(e) => Err(anyhow!("cannot lock {}: {e}", path.display())),
    }
}

/// The parsed file, empty when it does not exist yet. An unreadable or
/// unparsable file is an error, so it is never replaced by a partial one.
fn read_all(stats: &Path) -> Result<Vec<AppStats>> {
    let text = match fs::read_to_string(stats) {
        Ok(text) => text,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(anyhow!("cannot read {}: {e}", stats.display())),
    };
    match serde_json::from_str(&text) {
        Ok(apps) => Ok(apps),
        Err(e) => Err(anyhow!("cannot parse {}: {e}", stats.display())),
    }
}

/// `app`'s entry with its container rows replaced by `rows` and its totals
/// summed the way the supervisor sums them; `None` when nothing is left.
fn rebuilt_entry(app: &str, old: Option<AppStats>, rows: Vec<ProcessStats>) -> Option<AppStats> {
    let kept = old
        .map(|entry| entry.processes)
        .unwrap_or_default()
        .into_iter()
        .filter(|p| p.container.is_none());
    let mut manager = StatsManager::new();
    for process in kept.chain(rows) {
        manager.insert(process);
    }
    manager
        .get_all_stats()
        .into_iter()
        .find(|entry| entry.app == app)
}

/// Replace `app`'s container rows in `stats` with `rows`, leaving its
/// process rows and every other app's entry as they are.
#[allow(clippy::question_mark)]
pub fn replace_app_container_rows(stats: &Path, app: &str, rows: Vec<ProcessStats>) -> Result<()> {
    let _lock = match lock(stats) {
        Ok(lock) => lock,
        Err(e) => return Err(e),
    };
    let apps = match read_all(stats) {
        Ok(apps) => apps,
        Err(e) => return Err(e),
    };
    let (mut entries, old): (Vec<AppStats>, Vec<AppStats>) =
        apps.into_iter().partition(|entry| entry.app != app);
    if let Some(entry) = rebuilt_entry(app, old.into_iter().next(), rows) {
        entries.push(entry);
    }
    let json = match serde_json::to_string_pretty(&entries) {
        Ok(json) => json,
        Err(e) => return Err(anyhow!("cannot encode stats: {e}")),
    };
    crate::util::write_atomic(stats, json.as_bytes())
}

#[cfg(test)]
#[path = "stats_file_tests.rs"]
mod tests;
