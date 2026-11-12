//! The capture time at the front of each app log line.
//!
//! The supervisor writes every line it captures from a worker or copies
//! from a container as `<time> <text>`, the time in UTC to the second
//! (`2026-11-12T08:14:03Z`). `riku logs` and the dashboard read it back to
//! show when the line was written. `deploy.log` keeps its own
//! `[<local time with offset>] ` prefix, which is read here too.

use chrono::{DateTime, SecondsFormat, Utc};

/// `at` as written at the front of a log line.
pub fn format_stamp(at: DateTime<Utc>) -> String {
    at.to_rfc3339_opts(SecondsFormat::Secs, true)
}

/// `text` with the capture time `at` in front.
pub fn stamp_line(at: DateTime<Utc>, text: &str) -> String {
    format!("{} {text}", format_stamp(at))
}

/// The time a log line carries and the text after it, or `None` for a line
/// with no stamp, such as a `[WARN]` line of the deploy log.
pub fn split_stamp(line: &str) -> Option<(DateTime<Utc>, &str)> {
    if let Some(rest) = line.strip_prefix('[') {
        let (stamp, text) = match rest.split_once("] ") {
            Some(parts) => parts,
            None => return None,
        };
        return match DateTime::parse_from_str(stamp, "%Y-%m-%dT%H:%M:%S%z") {
            Ok(at) => Some((at.with_timezone(&Utc), text)),
            Err(_) => None,
        };
    }
    let (stamp, text) = match line.split_once(' ') {
        Some(parts) => parts,
        None => return None,
    };
    if !stamp.ends_with('Z') {
        return None;
    }
    match DateTime::parse_from_rfc3339(stamp) {
        Ok(at) => Some((at.with_timezone(&Utc), text)),
        Err(_) => None,
    }
}

#[cfg(test)]
#[path = "log_stamp_tests.rs"]
mod tests;
