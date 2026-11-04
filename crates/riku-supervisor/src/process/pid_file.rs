//! One empty marker file per live worker process, named
//! `<riku_root>/pids/<process_id>.<pid>`, so deploy can see a stopped
//! worker's process is gone without a round trip through `stats.json`.
//!
//! The supervisor creates the file right after the spawn and removes it once
//! the child has been reaped. A supervisor that dies without cleaning up
//! leaves files behind, so a reader also checks that the pid still answers
//! `kill(pid, 0)`.
//!
//! The process id is a `WorkerName` the supervisor built itself, which holds
//! no `/`, so the file always lands inside the pids directory.

use std::fs::{self, File};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use nix::errno::Errno;
use nix::sys::signal::kill;
use nix::unistd::Pid;
use riku_util::file_names::worker::WorkerName;

/// Where the supervisor keeps worker pid files.
pub fn worker_pid_dir(riku_root: &Path) -> PathBuf {
    riku_root.join("pids")
}

/// The marker of one spawned worker process. Dropping it removes the file.
#[derive(Debug)]
pub struct WorkerPidFile(PathBuf);

impl WorkerPidFile {
    pub fn create(dir: &Path, process_id: &str, pid: u32) -> Result<WorkerPidFile> {
        if let Err(e) = fs::create_dir_all(dir) {
            return Err(anyhow!("cannot create {}: {e}", dir.display()));
        }
        let path = dir.join(format!("{process_id}.{pid}"));
        match File::create(&path) {
            Ok(_) => Ok(WorkerPidFile(path)),
            Err(e) => Err(anyhow!("cannot create {}: {e}", path.display())),
        }
    }
}

impl Drop for WorkerPidFile {
    fn drop(&mut self) {
        if let Err(e) = fs::remove_file(&self.0) {
            tracing::warn!("cannot remove {}: {e}", self.0.display());
        }
    }
}

/// A worker that switched to another uid answers `EPERM`, which still means
/// the pid exists.
fn pid_exists(pid: i32) -> bool {
    match kill(Pid::from_raw(pid), None) {
        Ok(()) => true,
        Err(Errno::EPERM) => true,
        Err(_) => false,
    }
}

#[allow(clippy::question_mark)]
fn marker_of_app(file_name: &str, app: &str) -> Option<i32> {
    let (process_id, pid) = match file_name.rsplit_once('.') {
        Some(parts) => parts,
        None => return None,
    };
    match WorkerName::parse(process_id) {
        Some(name) if name.app == app => pid.parse().ok(),
        _ => None,
    }
}

/// Whether any worker process of `app` is still alive. Counts canary copies
/// spawned under a temporary ordinal too. A missing directory means the
/// supervisor never spawned anything.
#[allow(clippy::question_mark)]
pub fn app_has_live_worker(dir: &Path, app: &str) -> std::io::Result<bool> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(e),
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(e) => return Err(e),
        };
        let file_name = entry.file_name();
        let pid = file_name.to_str().and_then(|name| marker_of_app(name, app));
        if pid.is_some_and(pid_exists) {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
#[path = "pid_file_tests.rs"]
mod tests;
