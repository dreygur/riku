//! Watching `riku-nginx-reload.service` to confirm an nginx reload.
//!
//! The unit is started by a path unit watching one request file, which riku
//! writes after it changed the nginx config.
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
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use nix::time::{clock_gettime, ClockId};

use super::unit_state::{read_unit, UnitState};
use crate::config::{ReloadPrograms, RikuPaths};

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
pub(super) const NGINX_UNIT: &str = "riku-nginx-reload.service";

/// How nginx will be reloaded, decided before the config is written.
pub enum Baseline {
    /// The reload unit exists.
    Unit,
    /// No usable reload unit: run `nginx -s reload` directly.
    Direct,
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
        Baseline::Unit
    }
}

fn monotonic_micros() -> Result<u64> {
    match clock_gettime(ClockId::CLOCK_MONOTONIC) {
        Ok(now) => Ok(now.tv_sec() as u64 * 1_000_000 + now.tv_nsec() as u64 / 1_000),
        Err(e) => Err(anyhow!("cannot read the monotonic clock: {e}")),
    }
}

/// Start the reload unit by writing the request file `riku-nginx.path`
/// watches. The content is only there so each write is a modification.
fn request_reload(trigger: &Path) {
    if let Err(e) = fs::write(trigger, format!("{}\n", std::process::id())) {
        tracing::warn!(
            "cannot request a reload from {NGINX_UNIT} through {}: {e}",
            trigger.display()
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

fn unit_idle(state: &UnitState) -> bool {
    state.active_state == "inactive" || state.active_state == "failed"
}

/// Request a reload and wait for a run of the reload unit that started after
/// the config was written. While no such run has started (a run already in
/// flight, or a concurrent deploy's run, took the path unit's trigger), the
/// request is written again each time the unit is seen idle and
/// `RETRIGGER_AFTER` has passed since the last request. A trigger that lands
/// while the unit is active is lost, so one request is not enough, and
/// requesting while the unit is active is pointless. An older run is never
/// accepted.
#[allow(clippy::question_mark)]
pub fn await_unit(paths: &RikuPaths) -> Result<()> {
    let written_at = match monotonic_micros() {
        Ok(now) => now,
        Err(e) => return Err(e),
    };
    let trigger = paths.nginx_reload_trigger();
    request_reload(&trigger);
    let started = Instant::now();
    let mut last_trigger = started;
    loop {
        let state = match read_unit(&paths.reload_programs) {
            Ok(state) => state,
            Err(e) => return Err(e),
        };
        if let Some(outcome) = confirmed(&state, written_at) {
            return outcome;
        }
        if state.started_at < written_at
            && unit_idle(&state)
            && last_trigger.elapsed() >= RETRIGGER_AFTER
        {
            request_reload(&trigger);
            last_trigger = Instant::now();
        }
        if started.elapsed() >= RELOAD_WAIT {
            return Err(anyhow!(
                "nginx did not confirm the reload within {}s: {NGINX_UNIT} did not run after the config was written",
                RELOAD_WAIT.as_secs()
            ));
        }
        std::thread::sleep(PROBE_INTERVAL);
    }
}
