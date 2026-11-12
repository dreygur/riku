//! `riku __dump-state`: read-only export of the supervisor's in-memory
//! state matrix (port allocations, worker PIDs, deploy locks, router
//! state) as structured JSON, for operator visibility without
//! interrupting the running supervisor.
//!
//! # Security model
//!
//! Everything here is read from on-disk state the supervisor itself
//! already persists (`stats.json`, app `ENV` files, router configs, deploy
//! lock files): this command never connects to the running supervisor
//! process, so it can't disturb it, and it never blocks: the deploy-lock
//! check is a non-blocking probe (see `deploy::lock::is_locked`) that's
//! released immediately if it happens to succeed.
//!
//! App `ENV` files routinely hold customer secrets (`DATABASE_URL`,
//! `SECRET_KEY`, API tokens, ...). Those must never appear in this dump.
//! `ROUTING_ENV_ALLOWLIST` is a strict allowlist, not a blocklist: only
//! the handful of known, non-secret routing/port keys are ever read out of
//! an app's env map. Everything else is silently omitted, never just
//! masked.

use anyhow::Result;

use crate::config::RikuPaths;

mod builder;
mod kinds;
mod route;
mod supervisor_files;
mod types;

pub use builder::{build_app_entry, build_state_dump, extract_routing_fields};
pub use route::RouteState;
pub use types::{
    AppStateEntry, EngineInfo, LockState, ServiceStateEntry, StateDump, UnitKind, WorkerStateEntry,
};

/// Dump the current supervisor state matrix as pretty-printed JSON to stdout.
#[allow(clippy::question_mark)]
pub fn cmd_dump_state(paths: &RikuPaths) -> Result<()> {
    let dump = match build_state_dump(paths) {
        Ok(dump) => dump,
        Err(e) => return Err(e),
    };
    let json_text = match serde_json::to_string_pretty(&dump) {
        Ok(text) => text,
        Err(e) => return Err(e.into()),
    };
    println!("{json_text}");
    Ok(())
}

/// The read-only state matrix as a JSON value, for the dashboard API. Reuses
/// the same secret-safe builder as `cmd_dump_state` (app env is allowlisted to
/// routing/port keys, so secrets never appear).
#[allow(clippy::question_mark)]
pub fn state_json(paths: &RikuPaths) -> Result<serde_json::Value> {
    let dump = match build_state_dump(paths) {
        Ok(dump) => dump,
        Err(e) => return Err(e),
    };
    match serde_json::to_value(dump) {
        Ok(val) => Ok(val),
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
#[path = "dump_state_tests.rs"]
mod tests;
