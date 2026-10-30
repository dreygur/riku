//! The audit log of agent actions: one JSON object per line.
//!
//! Security model: `action` and `app` come from the agent argv, which the
//! server owner wrote today. They may come from the client once agent
//! requests over ssh are read, so entries are encoded as JSON strings.
//! Control characters are escaped, which keeps every entry on one line and
//! stops a value from forging another entry.

use std::fs::File;
use std::io::{Result, Write};
use std::path::Path;

use serde_json::json;

use super::state::{open_private, Access};
use super::types::is_mutating;

const AUDIT_FILE: &str = "audit.log";

/// One thing an agent asked for and how it ended.
pub struct AuditEntry<'a> {
    pub agent: &'a str,
    pub action: &'a str,
    pub app: &'a str,
    pub success: bool,
}

/// Opens the audit log of `state` for appending.
pub fn open_audit_log(state: &Path) -> Result<File> {
    open_private(&state.join(AUDIT_FILE), Access::Append)
}

/// The audit log opened ahead of `command`, when the command changes
/// something. An action that cannot be recorded must not run, so a failure
/// here is the caller's reason to refuse. Read only commands return `None`.
pub fn open_before_mutation(state: &Path, command: &str) -> Result<Option<File>> {
    if !is_mutating(command) {
        return Ok(None);
    }
    match open_audit_log(state) {
        Ok(file) => Ok(Some(file)),
        Err(e) => Err(e),
    }
}

fn entry_line(entry: &AuditEntry) -> String {
    let status = if entry.success { "success" } else { "failed" };
    let line = json!({
        "time": chrono::Local::now().to_rfc3339(),
        "agent": entry.agent,
        "action": entry.action,
        "app": entry.app,
        "status": status,
    });
    format!("{line}\n")
}

/// Appends `entry` to `file`. The line goes out in a single write so
/// concurrent agents do not interleave.
pub fn write_entry(file: &mut File, entry: &AuditEntry) -> Result<()> {
    file.write_all(entry_line(entry).as_bytes())
}

/// Records `entry` in `opened`, or in a freshly opened log when `opened` is
/// `None`. A log that cannot be written is reported, not fatal: the action
/// has already run.
pub fn log_agent_action(state: &Path, opened: Option<File>, entry: &AuditEntry) {
    let file = match opened {
        Some(file) => Ok(file),
        None => open_audit_log(state),
    };
    let written = match file {
        Ok(mut file) => write_entry(&mut file, entry),
        Err(e) => Err(e),
    };
    if let Err(e) = written {
        tracing::warn!("agent audit log not written: {e}");
    }
}

#[cfg(test)]
#[path = "audit_tests.rs"]
mod tests;
