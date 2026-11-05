//! Which init system the host runs, found by capability, and the
//! `host.toml` that records it for later commands.
//!
//! Security model: runs as root during init. The probes only stat paths and
//! read `PATH`; nothing is executed. `host.toml` is written into
//! `/var/lib/riku` only after [`check_write_dir`] confirms that directory
//! is real, root owned and closed to group and other writers, so no other
//! account can redirect the write. `PATH` entries that are not absolute are
//! skipped, so the working directory never decides the answer.

use std::env;
use std::ffi::OsStr;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use anyhow::{anyhow, Result};
use riku_util::fs::write_atomic;

use crate::config::host_file::HostFacts;
pub use crate::config::host_file::InitSystem;
use crate::config::HOST_FILE;
use crate::util::echo;

use super::nginx_host::{check_write_dir, ROOT_UID};

const SYSTEMD_RUN_DIR: &str = "run/systemd/system";
const OPENRC_RUN_DIR: &str = "run/openrc";
const OPENRC_RUNNER: &str = "openrc-run";
const HOST_FILE_MODE: u32 = 0o644;

fn is_executable_file(path: &Path) -> bool {
    match path.metadata() {
        Ok(meta) => meta.is_file() && meta.permissions().mode() & 0o111 != 0,
        Err(_) => false,
    }
}

fn on_path(program: &str, path_var: &OsStr) -> bool {
    env::split_paths(path_var)
        .filter(|dir| dir.is_absolute())
        .any(|dir| is_executable_file(&dir.join(program)))
}

fn detect_with(root: &Path, path_var: &OsStr) -> InitSystem {
    if root.join(SYSTEMD_RUN_DIR).is_dir() {
        return InitSystem::Systemd;
    }
    if on_path(OPENRC_RUNNER, path_var) && root.join(OPENRC_RUN_DIR).is_dir() {
        return InitSystem::OpenRc;
    }
    InitSystem::Other
}

/// The init system of the host whose filesystem root is `root`. Systemd when
/// `<root>/run/systemd/system` is a directory. OpenRC needs both
/// `openrc-run` on `PATH` and `<root>/run/openrc` as a directory, because
/// the binary alone is installed on hosts that boot something else.
pub fn detect_init(root: &Path) -> InitSystem {
    let path_var = env::var_os("PATH").unwrap_or_default();
    detect_with(root, &path_var)
}

/// Write `facts` to `file` as a root owned `0644` file. The directory
/// holding it must pass [`check_write_dir`] for `owner`.
fn write_host_file(file: &Path, owner: u32, facts: &HostFacts) -> Result<()> {
    let Some(dir) = file.parent() else {
        return Err(anyhow!("{} has no parent directory", file.display()));
    };
    if let Err(e) = check_write_dir(dir, owner) {
        return Err(anyhow!("cannot record host facts: {e}"));
    }
    if let Err(e) = write_atomic(file, facts.to_toml().as_bytes()) {
        return Err(anyhow!("cannot write {}: {e}", file.display()));
    }
    match fs::set_permissions(file, fs::Permissions::from_mode(HOST_FILE_MODE)) {
        Ok(()) => Ok(()),
        Err(e) => Err(anyhow!("cannot chmod {}: {e}", file.display())),
    }
}

/// Detect the init system of this host and record it in `host.toml`. Root
/// only, and run after `/var/lib/riku` exists. Writing the real file is
/// covered only by running init on a host.
pub(super) fn record_init_system() -> Result<InitSystem> {
    let init = detect_init(Path::new("/"));
    if let Err(e) = write_host_file(Path::new(HOST_FILE), ROOT_UID, &HostFacts { init }) {
        return Err(anyhow!("init system step failed: {e}"));
    }
    echo(
        &format!("      ✓ {HOST_FILE} records init system {}", init.as_str()),
        "green",
    );
    Ok(init)
}

#[cfg(test)]
#[path = "init_system_tests.rs"]
mod tests;
