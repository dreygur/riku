//! Container events streamed from the engine.
//!
//! Security model: event lines are untrusted engine output, only a container
//! id is taken from them and it must pass `ContainerId::try_from`. Filters are
//! fixed strings, each passed as its own argv element.

use std::ffi::OsString;
use std::process::Command;

use anyhow::Result;
use serde_json::Value;

use super::bounded::capture;
use super::env::apply_env;
use super::{ContainerId, Engine};
use crate::types::labels;

/// `destroy` makes the watch inspect a removed container, find it gone and
/// forget it. Without it a removed copy stays listed, in its last state,
/// until the next full listing.
const EVENT_NAMES: [&str; 4] = ["start", "die", "health_status", "destroy"];

impl Engine {
    /// Check whether the engine supports streaming events.
    ///
    /// Engines without `events` make the supervisor fall back to polling.
    pub fn has_events(&self) -> bool {
        let args = [OsString::from("events"), OsString::from("--help")];
        match capture(
            self.program(),
            self.env(),
            &args,
            self.settings().call_timeout,
        ) {
            Ok(out) => out.status.success(),
            Err(_) => false,
        }
    }

    /// A command for the caller to spawn and read container events line by line.
    ///
    /// Filters keep only containers carrying the `riku.app` label and lifecycle
    /// events in [`EVENT_NAMES`].
    pub fn events_command(&self) -> Command {
        let mut command = Command::new(self.program());
        apply_env(&mut command, self.env());
        command.arg("events");
        command.args(["--format", "{{json .}}"]);
        command.args(["--filter", "type=container"]);
        command.args(["--filter", &format!("label={}", labels::APP)]);
        for event in EVENT_NAMES {
            command.args(["--filter", &format!("event={event}")]);
        }
        command
    }

    /// Full ids of every container that carries the `riku.app` label, running or not.
    pub fn list_riku(&self) -> Result<Vec<ContainerId>> {
        let filter = format!("label={}", labels::APP);
        self.list_with_filter(Some(&filter))
    }
}

/// Parse a container identifier from an engine event JSON line.
///
/// Looks for `Actor.ID`, falling back to top level `ID` or `id`. Returns
/// `None` when the line is not JSON, does not carry an identifier, or the
/// identifier fails validation.
pub fn event_container(line: &str) -> Option<ContainerId> {
    let doc: Value = match serde_json::from_str(line) {
        Ok(doc) => doc,
        Err(_) => return None,
    };
    let raw = [&doc["Actor"]["ID"], &doc["ID"], &doc["id"]]
        .into_iter()
        .find_map(Value::as_str);
    raw.and_then(|raw| ContainerId::try_from(raw).ok())
}

#[cfg(test)]
#[path = "events_tests.rs"]
mod tests;
