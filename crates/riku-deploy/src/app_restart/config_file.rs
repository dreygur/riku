//! Opening the worker config an enabled entry leads to, for a restart that
//! reads it or advances its mtime.
//!
//! Security model: `workers-enabled` is the deploy user's own directory, but
//! an entry in it is only trusted as far as it leads to a regular file
//! directly in `workers-available` owned by this user. The file is opened
//! read only, `O_NOFOLLOW | O_NONBLOCK`, so a planted link at the final
//! component or a FIFO is refused without being followed or blocking.

// Project rule 16 forbids `?`, and this lint asks for exactly that rewrite.
#![allow(clippy::question_mark)]

use std::fs::{self, File};
use std::io::Read;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{anyhow, Result};
use riku_supervisor::config::WorkerConfig;

use crate::config::RikuPaths;

/// The config file `config` leads to: the target of an enabled link, or the
/// file itself. It must sit directly in `workers_available`; the returned
/// path is under the canonical form of that directory, so only its last
/// component can still be a link, and opening refuses that.
fn config_in_available(config: &Path, paths: &RikuPaths) -> Result<PathBuf> {
    let target = match fs::read_link(config) {
        Ok(target) => match config.parent() {
            Some(dir) => dir.join(target),
            None => target,
        },
        Err(_) => config.to_path_buf(),
    };
    let (dir, name) = match (target.parent(), target.file_name()) {
        (Some(dir), Some(name)) => (dir, name),
        _ => return Err(anyhow!("{} is not a worker config", config.display())),
    };
    let available = fs::canonicalize(&paths.workers_available);
    match (fs::canonicalize(dir), available) {
        (Ok(dir), Ok(available)) if dir == available => Ok(available.join(name)),
        _ => Err(anyhow!(
            "{} does not lead to a config in {}",
            config.display(),
            paths.workers_available.display()
        )),
    }
}

/// The config `config` leads to, opened and checked, with its path.
fn open_config(config: &Path, paths: &RikuPaths) -> Result<(PathBuf, File)> {
    let target = match config_in_available(config, paths) {
        Ok(target) => target,
        Err(e) => return Err(e),
    };
    let opened = File::options()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
        .open(&target);
    let file = match opened {
        Ok(file) => file,
        Err(e) => return Err(anyhow!("cannot open {}: {e}", target.display())),
    };
    let meta = match file.metadata() {
        Ok(meta) => meta,
        Err(e) => return Err(anyhow!("cannot stat {}: {e}", target.display())),
    };
    if !meta.is_file() {
        return Err(anyhow!("{} is not a regular file", target.display()));
    }
    if meta.uid() != nix::unistd::geteuid().as_raw() {
        return Err(anyhow!("{} is not owned by this user", target.display()));
    }
    Ok((target, file))
}

/// Advance the mtime of the config `config` leads to, so the supervisor
/// reloads that worker.
pub(super) fn bump_mtime(config: &Path, paths: &RikuPaths) -> Result<()> {
    let (target, file) = match open_config(config, paths) {
        Ok(opened) => opened,
        Err(e) => return Err(e),
    };
    match file.set_modified(SystemTime::now()) {
        Ok(()) => Ok(()),
        Err(e) => Err(anyhow!("cannot touch {}: {e}", target.display())),
    }
}

/// The worker config `config` leads to, parsed.
pub(super) fn read_config(config: &Path, paths: &RikuPaths) -> Result<WorkerConfig> {
    let (target, mut file) = match open_config(config, paths) {
        Ok(opened) => opened,
        Err(e) => return Err(e),
    };
    let mut text = String::new();
    if let Err(e) = file.read_to_string(&mut text) {
        return Err(anyhow!("cannot read {}: {e}", target.display()));
    }
    match toml::from_str(&text) {
        Ok(parsed) => Ok(parsed),
        Err(e) => Err(anyhow!("cannot parse {}: {e}", target.display())),
    }
}
