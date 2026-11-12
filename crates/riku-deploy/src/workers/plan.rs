//! Everything a deploy checks before it touches a worker config: the pinned
//! port rule, the refused app socket, the process list and each kind's copy
//! count.
//! A deploy refused here leaves every running worker as it was.

// Project rule 16 forbids `?`, and this lint asks for exactly that rewrite.
#![allow(clippy::question_mark)]

use std::collections::HashMap;

use anyhow::Result;

use super::pinned::check_pinned_port_with;
use super::socket::refuse_app_socket;
use super::web_env::is_web_facing;
use super::{read_scaling_count, ONE_SHOT_KINDS, WEB_KIND};
use crate::config::RikuPaths;

/// A long running process kind and what the deploy starts for it.
pub(super) struct PlannedKind {
    pub(super) kind: String,
    /// The declared command, or the plugin's start command when the
    /// declared one is empty.
    pub(super) command: String,
    /// Copies wanted, from SCALING (1 when not listed).
    pub(super) count: u32,
}

/// What a deploy will run, checked and complete.
pub(super) struct WorkerPlan {
    /// Every declared entry, one shot kinds included; a running web facing
    /// kind not in here is stopped.
    pub(super) entries: HashMap<String, String>,
    pub(super) kinds: Vec<PlannedKind>,
}

/// Check the deploy's processes and read their copy counts. `None` when the
/// app declares no processes and the plugin gives no start command.
pub(super) fn plan_workers(
    app: &str,
    processes: Option<&HashMap<String, String>>,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
    start_cmd: Option<&str>,
    in_use: impl Fn(u16) -> bool,
) -> Result<Option<WorkerPlan>> {
    if let Err(e) = check_pinned_port_with(app, env, paths, in_use) {
        return Err(e);
    }
    let entries = match (processes, start_cmd) {
        (Some(declared), _) if !declared.is_empty() => declared.clone(),
        (_, Some(cmd)) => HashMap::from([(WEB_KIND.to_string(), cmd.to_string())]),
        (_, None) => return Ok(None),
    };
    if entries.keys().any(|kind| is_web_facing(kind)) {
        if let Err(e) = refuse_app_socket(env) {
            return Err(e);
        }
    }
    let mut kinds = Vec::new();
    for (kind, command) in &entries {
        if ONE_SHOT_KINDS.contains(&kind.as_str()) {
            continue;
        }
        let count = match read_scaling_count(paths, app, kind) {
            Ok(count) => count,
            Err(e) => return Err(e),
        };
        let command = match start_cmd {
            Some(start) if command.is_empty() => start.to_string(),
            _ => command.clone(),
        };
        kinds.push(PlannedKind {
            kind: kind.clone(),
            command,
            count,
        });
    }
    Ok(Some(WorkerPlan { entries, kinds }))
}

#[cfg(test)]
#[path = "plan_tests.rs"]
mod tests;
