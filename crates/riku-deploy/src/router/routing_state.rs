//! Riku's record of how each app is routed: its mode, the port it holds and
//! how its fallback certificate went. Lives in `<riku root>/routing/`.
//!
//! The record is what makes a port riku picked stay the app's port, keeps
//! two apps from picking the same one, and lets path mode and the push
//! output read an app's routing without resolving every app again. Every
//! read, change and write happens under one server wide `flock`, held only
//! for that short section.
//!
//! Security: only riku writes this file, as the deploy user. It is parsed
//! strictly anyway, so a hand edit with an unknown field is refused rather
//! than half read.

use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{self, ErrorKind};
use std::os::unix::io::AsRawFd;
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use crate::config::RikuPaths;

const STATE_DIR: &str = "routing";
const STATE_FILE: &str = "state.toml";
const LOCK_FILE: &str = "lock";

/// How an app is reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoutingMode {
    /// By its own domains or the fallback domain.
    Domain,
    /// On a port of its own, by any host name.
    Port,
    /// Under `/<app>/` on the shared port.
    Path,
}

/// How the fallback domain's certificate went.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum TlsState {
    Issued,
    /// The issue failed at this unix time; no retry for an hour.
    FailedAt(u64),
    /// Caddy issues it on its own.
    ByCaddy,
}

/// One app's routing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub mode: RoutingMode,
    /// The port the app answers on besides its domain, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    /// True when riku picked `port` from the range, so it may free it.
    #[serde(default)]
    pub picked: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls: Option<TlsState>,
    /// The name the certificate pair riku issued for the fallback covers, so a
    /// pair is never served for another name. `None`: no riku issued pair.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cert_for: Option<String>,
    /// The domains the router serves the app under, as resolved for the
    /// last apply: its own or the one fallback name. Empty when it has
    /// none, as in path mode. The dashboard reads it to show where the app answers now,
    /// which the current settings may no longer give.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub domains: Vec<String>,
    /// True when `domains` is the fallback name.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub fallback: bool,
}

/// Every app's entry.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoutingState {
    #[serde(default)]
    apps: BTreeMap<String, Entry>,
}

fn state_dir(paths: &RikuPaths) -> PathBuf {
    paths.riku_root.join(STATE_DIR)
}

impl RoutingState {
    /// Read the record; a missing file is an empty record.
    pub fn load(paths: &RikuPaths) -> Result<RoutingState> {
        let file = state_dir(paths).join(STATE_FILE);
        let raw = match fs::read_to_string(&file) {
            Ok(raw) => raw,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(RoutingState::default()),
            Err(e) => return Err(anyhow!("failed to read {}: {e}", file.display())),
        };
        toml::from_str(&raw).map_err(|e| anyhow!("{}: {e}", file.display()))
    }

    fn save(&self, paths: &RikuPaths) -> Result<()> {
        let file = state_dir(paths).join(STATE_FILE);
        let text = match toml::to_string(self) {
            Ok(text) => text,
            Err(e) => return Err(anyhow!("failed to encode routing state: {e}")),
        };
        let tmp = file.with_extension("toml.new");
        if let Err(e) = fs::write(&tmp, text) {
            return Err(anyhow!("failed to write {}: {e}", tmp.display()));
        }
        fs::rename(&tmp, &file).map_err(|e| anyhow!("failed to replace {}: {e}", file.display()))
    }

    pub fn entry(&self, app: &str) -> Option<&Entry> {
        self.apps.get(app)
    }

    pub fn set_entry(&mut self, app: &str, entry: Entry) {
        self.apps.insert(app.to_string(), entry);
    }

    pub fn remove(&mut self, app: &str) {
        self.apps.remove(app);
    }

    /// Every app and its entry, in name order.
    pub fn entries(&self) -> impl Iterator<Item = (&str, &Entry)> {
        self.apps.iter().map(|(app, entry)| (app.as_str(), entry))
    }
}

fn lock_exclusive(file: &fs::File) -> io::Result<()> {
    loop {
        let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) };
        if rc == 0 {
            return Ok(());
        }
        let err = io::Error::last_os_error();
        if err.raw_os_error() != Some(libc::EINTR) {
            return Err(err);
        }
    }
}

/// Run `change` on the record under the server wide routing lock and save
/// the result when it succeeds. A failed `change` writes nothing. The lock
/// is released when the lock file closes.
#[allow(clippy::question_mark)]
pub fn with_routing_lock<T>(
    paths: &RikuPaths,
    change: impl FnOnce(&mut RoutingState) -> Result<T>,
) -> Result<T> {
    let dir = state_dir(paths);
    if let Err(e) = fs::create_dir_all(&dir) {
        return Err(anyhow!("failed to create {}: {e}", dir.display()));
    }
    let lock_path = dir.join(LOCK_FILE);
    let lock = match OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lock_path)
    {
        Ok(lock) => lock,
        Err(e) => return Err(anyhow!("failed to open {}: {e}", lock_path.display())),
    };
    if let Err(e) = lock_exclusive(&lock) {
        return Err(anyhow!("failed to lock {}: {e}", lock_path.display()));
    }
    let mut state = match RoutingState::load(paths) {
        Ok(state) => state,
        Err(e) => return Err(e),
    };
    let value = match change(&mut state) {
        Ok(value) => value,
        Err(e) => return Err(e),
    };
    state.save(paths).map(|()| value)
}

/// Drop a destroyed app's entry, freeing its port. A failure is logged, not
/// returned: the app is already gone, and a stale entry only holds a port.
pub fn forget(paths: &RikuPaths, app: &str) {
    let dropped = with_routing_lock(paths, |state| {
        state.remove(app);
        Ok(())
    });
    if let Err(e) = dropped {
        tracing::warn!("cannot drop the routing entry of '{app}': {e:#}");
    }
}

/// Put back `app`'s entry from before an apply that failed, so the record
/// keeps saying how the app is really routed: a mode switch that did not
/// reach the router must not look done to the next deploy. `None` drops
/// the entry a first, failed apply made.
pub fn restore(paths: &RikuPaths, app: &str, previous: Option<Entry>) {
    let restored = with_routing_lock(paths, |state| {
        match previous {
            Some(entry) => state.set_entry(app, entry),
            None => state.remove(app),
        }
        Ok(())
    });
    if let Err(e) = restored {
        tracing::warn!("cannot restore the routing entry of '{app}': {e:#}");
    }
}

#[cfg(test)]
#[path = "routing_state_tests.rs"]
mod tests;
