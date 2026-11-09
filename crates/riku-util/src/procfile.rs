//! Procfile parsing utilities.
//!
//! Parses Heroku-style Procfile entries, validates cron expressions,
//! and enforces the rule that WSGI workers supersede plain web workers.
//!
//! Cron entries use the **5-field Unix cron** format
//! (`minute hour day-of-month month day-of-week`) followed by the command to
//! run. Schedules are validated by the supervisor's cron engine
//! ([`crate::supervisor::cron::validate_cron_expression`]) so there is exactly
//! one definition of what a valid schedule is, and they are interpreted in
//! **UTC** at run time (see that module for the time-zone contract).

use anyhow::{anyhow, Result};
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use super::display::echo;
use crate::cron::validate_cron_expression;

/// Pre-compiled environment variable expansion regex (also used by `env.rs`).
pub(crate) static ENVVAR_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\$(\w+|\{([^}]*)\})").unwrap());

/// Validate the cron portion of a Procfile `cron` entry.
///
/// The command is `<5 schedule fields> <command…>`; only the leading schedule
/// is validated, using the shared supervisor cron engine. Returns `false` when
/// there are fewer than five fields or the schedule is malformed/unsatisfiable.
/// Public so `riku.toml` `[cron]` entries get the same check.
pub fn is_valid_cron_command(command: &str) -> bool {
    let fields: Vec<&str> = command.split_whitespace().collect();
    if fields.len() < 5 {
        return false;
    }
    let schedule = fields[..5].join(" ");
    validate_cron_expression(&schedule)
}

/// A process kind names worker config and log files, so it is limited to
/// letters, digits, `_` and `-`. It never holds the `@` or `.` that
/// [`crate::file_names::worker`] splits a worker name on.
pub fn is_valid_kind(kind: &str) -> bool {
    !kind.is_empty()
        && kind
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// One Procfile line as `(kind, command)`, `None` for a blank or comment
/// line, or why the line is invalid.
fn parse_line(line: &str) -> Result<Option<(String, String)>, String> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return Ok(None);
    }
    let Some((kind, command)) = line.split_once(':') else {
        return Err("expected `kind: command`".to_string());
    };
    let (kind, command) = (kind.trim(), command.trim());
    if !is_valid_kind(kind) {
        return Err(format!("invalid process type {kind:?}"));
    }
    // The supervisor's cron engine decides, so the Procfile and the
    // scheduler agree on what a valid schedule is.
    if kind.starts_with("cron") && !is_valid_cron_command(command) {
        return Err("invalid cron schedule".to_string());
    }
    Ok(Some((kind.to_string(), command.to_string())))
}

/// Parse a Heroku-style Procfile. Skips comments and blank lines, so a file
/// with only those has no processes. Any other line that is not a valid
/// `kind: command` fails the whole parse: deploying part of a broken
/// Procfile would run something the author did not write. WSGI trumps web
/// workers. Returns `None` if the file is missing.
pub fn parse_procfile(filename: &Path) -> Result<Option<HashMap<String, String>>> {
    if !filename.exists() {
        return Ok(None);
    }

    let content = match fs::read_to_string(filename) {
        Ok(content) => content,
        Err(e) => return Err(anyhow!("failed to read {}: {e}", filename.display())),
    };
    let mut workers: HashMap<String, String> = HashMap::new();

    for (index, line) in content.lines().enumerate() {
        let (kind, command) = match parse_line(line) {
            Ok(Some(entry)) => entry,
            Ok(None) => continue,
            Err(reason) => {
                return Err(anyhow!(
                    "Procfile line {}: {reason}: {:?}",
                    index + 1,
                    line.trim()
                ))
            }
        };
        if workers.contains_key(&kind) {
            echo(
                &format!(
                    "Warning: found multiple {} workers, only the last one will be used.",
                    kind
                ),
                "yellow",
            );
        }
        workers.insert(kind, command);
    }

    // WSGI trumps regular web workers
    if (workers.contains_key("wsgi")
        || workers.contains_key("jwsgi")
        || workers.contains_key("rwsgi"))
        && workers.contains_key("web")
    {
        echo(
            "Warning: found both 'wsgi' and 'web' workers, disabling 'web'",
            "yellow",
        );
        workers.remove("web");
    }

    Ok(Some(workers))
}

#[cfg(test)]
#[path = "procfile_tests.rs"]
mod tests;
