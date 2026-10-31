//! Watching `riku-nginx-reload.service` to confirm an nginx reload.
//!
//! The unit is started by a path unit when riku changes the nginx config.
//! A reload counts as confirmed only by a run that started after riku wrote
//! the config, finished, and exited 0. A run already in flight when the
//! config was written, or one a concurrent deploy triggered earlier, may have
//! read the old config, so it is never accepted.
//!
//! "Confirmed" here means the reload signal was delivered and accepted
//! (`systemctl reload nginx` exited 0). It does not prove nginx finished
//! loading the config.
//!
//! Security: the programs and arguments are fixed. Timestamps come from
//! systemd and are only compared, never used in a command line.

use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant, SystemTime};

use anyhow::{anyhow, Result};
use nix::time::{clock_gettime, ClockId};

use crate::config::{ReloadPrograms, RikuPaths};
use crate::util::spawn::output_retrying_etxtbsy;

#[cfg(not(test))]
const RELOAD_WAIT: Duration = Duration::from_secs(30);
#[cfg(test)]
const RELOAD_WAIT: Duration = Duration::from_secs(1);
#[cfg(not(test))]
const PROBE_INTERVAL: Duration = Duration::from_millis(500);
#[cfg(test)]
const PROBE_INTERVAL: Duration = Duration::from_millis(50);
#[cfg(not(test))]
const RETRIGGER_AFTER: Duration = Duration::from_secs(5);
#[cfg(test)]
const RETRIGGER_AFTER: Duration = Duration::from_millis(200);
const NGINX_UNIT: &str = "riku-nginx-reload.service";
const UNIT_PROPERTIES: &str = "ExecMainStartTimestampMonotonic,ExecMainExitTimestampMonotonic,ExecMainStatus,LoadState,ActiveState";

/// How nginx will be reloaded, decided before the config is written.
pub enum Baseline {
    /// The reload unit exists. `busy` is whether it was running at the time.
    Unit { busy: bool },
    /// No usable reload unit: run `nginx -s reload` directly.
    Direct,
}

struct UnitState {
    exit_ok: bool,
    load_state: String,
    active_state: String,
    started_at: u64,
    exited_at: u64,
    status: String,
}

impl UnitState {
    fn running(&self) -> bool {
        matches!(
            self.active_state.as_str(),
            "active" | "activating" | "reloading"
        )
    }
}

fn parse_timestamp(key: &str, raw: &str) -> u64 {
    if raw.is_empty() {
        return 0;
    }
    match raw.parse() {
        Ok(timestamp) => timestamp,
        Err(_) => {
            tracing::debug!("{NGINX_UNIT}: {key} is not a number, treating it as 0");
            0
        }
    }
}

fn read_unit(programs: &ReloadPrograms) -> Result<UnitState> {
    let output = match output_retrying_etxtbsy(
        Command::new(&programs.systemctl)
            .args(["show", "-p", UNIT_PROPERTIES])
            .arg(NGINX_UNIT),
    ) {
        Ok(output) => output,
        Err(e) => return Err(anyhow!("could not run systemctl: {e}")),
    };
    let text = String::from_utf8_lossy(&output.stdout);
    let value = |key: &str| {
        text.lines()
            .find_map(|line| {
                line.strip_prefix(key)
                    .and_then(|rest| rest.strip_prefix('='))
            })
            .unwrap_or_default()
            .trim()
            .to_string()
    };
    Ok(UnitState {
        exit_ok: output.status.success(),
        load_state: value("LoadState"),
        active_state: value("ActiveState"),
        started_at: parse_timestamp(
            "ExecMainStartTimestampMonotonic",
            &value("ExecMainStartTimestampMonotonic"),
        ),
        exited_at: parse_timestamp(
            "ExecMainExitTimestampMonotonic",
            &value("ExecMainExitTimestampMonotonic"),
        ),
        status: value("ExecMainStatus"),
    })
}

impl Baseline {
    /// Decide how nginx will be reloaded. Call it before the config is
    /// written. A missing or failing systemctl, a failed query, an empty
    /// `LoadState` or a unit that is not found (no systemd, as on a container
    /// host) all select a direct reload.
    pub fn capture(programs: &ReloadPrograms) -> Baseline {
        let state = match read_unit(programs) {
            Ok(state) => state,
            Err(e) => {
                tracing::debug!("reloading nginx directly: {e}");
                return Baseline::Direct;
            }
        };
        if !state.exit_ok || state.load_state.is_empty() || state.load_state == "not-found" {
            tracing::debug!(
                "reloading nginx directly: systemctl gave no usable {NGINX_UNIT} (exit ok: {}, LoadState '{}')",
                state.exit_ok,
                state.load_state
            );
            return Baseline::Direct;
        }
        Baseline::Unit {
            busy: state.running(),
        }
    }
}

fn monotonic_micros() -> Result<u64> {
    match clock_gettime(ClockId::CLOCK_MONOTONIC) {
        Ok(now) => Ok(now.tv_sec() as u64 * 1_000_000 + now.tv_nsec() as u64 / 1_000),
        Err(e) => Err(anyhow!("cannot read the monotonic clock: {e}")),
    }
}

fn touch_watched_dir(dir: &Path) {
    let touched = fs::File::open(dir).and_then(|handle| handle.set_modified(SystemTime::now()));
    if let Err(e) = touched {
        tracing::warn!(
            "cannot retrigger {NGINX_UNIT} through {}: {e}",
            dir.display()
        );
    }
}

fn confirmed(state: &UnitState, written_at: u64) -> Option<Result<()>> {
    let finished_run = state.started_at >= written_at && state.exited_at >= state.started_at;
    if !finished_run {
        return None;
    }
    if state.status == "0" {
        return Some(Ok(()));
    }
    Some(Err(anyhow!(
        "nginx reload failed: {NGINX_UNIT} exited with status {}",
        state.status
    )))
}

/// Wait for a run of the reload unit that started after the config was
/// written. When the unit was busy at write time and its run ends without a
/// new one starting, the watched directory is touched once to make the path
/// unit fire again.
#[allow(clippy::question_mark)]
pub fn await_unit(paths: &RikuPaths, busy: bool) -> Result<()> {
    let written_at = match monotonic_micros() {
        Ok(now) => now,
        Err(e) => return Err(e),
    };
    let started = Instant::now();
    let mut retriggered = false;
    loop {
        let state = match read_unit(&paths.reload_programs) {
            Ok(state) => state,
            Err(e) => return Err(e),
        };
        if let Some(outcome) = confirmed(&state, written_at) {
            return outcome;
        }
        let elapsed = started.elapsed();
        if busy && !retriggered && elapsed >= RETRIGGER_AFTER && !state.running() {
            touch_watched_dir(&paths.nginx_root);
            retriggered = true;
        }
        if elapsed >= RELOAD_WAIT {
            return Err(anyhow!(
                "nginx did not confirm the reload within {}s: {NGINX_UNIT} did not run after the config was written",
                RELOAD_WAIT.as_secs()
            ));
        }
        std::thread::sleep(PROBE_INTERVAL);
    }
}
