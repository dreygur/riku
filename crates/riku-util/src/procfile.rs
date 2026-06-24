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
fn is_valid_cron_command(command: &str) -> bool {
    let fields: Vec<&str> = command.split_whitespace().collect();
    if fields.len() < 5 {
        return false;
    }
    let schedule = fields[..5].join(" ");
    validate_cron_expression(&schedule)
}

/// Parse a Heroku-style Procfile. Skip comments/blanks. Validate cron entries.
/// WSGI trumps web workers. Returns None if file missing.
pub fn parse_procfile(filename: &Path) -> Option<HashMap<String, String>> {
    if !filename.exists() {
        return None;
    }

    let content = fs::read_to_string(filename).ok()?;
    let mut workers: HashMap<String, String> = HashMap::new();

    for (line_number, line) in content.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(colon_pos) = line.find(':') {
            let kind = line[..colon_pos].trim().to_string();
            let command = line[colon_pos + 1..].trim().to_string();

            // Validate cron entries through the shared supervisor cron engine
            // so the Procfile and the scheduler agree on what is valid (ranges,
            // lists, steps are all accepted; out-of-range and unsatisfiable
            // schedules are rejected).
            if kind.starts_with("cron") && !is_valid_cron_command(&command) {
                echo(
                    &format!(
                        "Warning: misformatted Procfile entry '{}' at line {}",
                        line, line_number
                    ),
                    "yellow",
                );
                continue;
            }

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
        } else {
            echo(
                &format!(
                    "Warning: misformatted Procfile entry '{}' at line {}",
                    line, line_number
                ),
                "yellow",
            );
        }
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

    Some(workers)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_parse_procfile_basic() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "web: python app.py").unwrap();
        writeln!(f, "worker: celery -A tasks").unwrap();
        let workers = parse_procfile(f.path()).unwrap();
        assert_eq!(workers.get("web").unwrap(), "python app.py");
        assert_eq!(workers.get("worker").unwrap(), "celery -A tasks");
    }

    #[test]
    fn test_parse_procfile_comments_and_blanks() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "# This is a comment").unwrap();
        writeln!(f).unwrap();
        writeln!(f, "web: python app.py").unwrap();
        writeln!(f).unwrap();
        writeln!(f, "# Another comment").unwrap();
        let workers = parse_procfile(f.path()).unwrap();
        assert_eq!(workers.len(), 1);
        assert_eq!(workers.get("web").unwrap(), "python app.py");
    }

    #[test]
    fn test_parse_procfile_wsgi_trumps_web() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "web: python app.py").unwrap();
        writeln!(f, "wsgi: app:application").unwrap();
        let workers = parse_procfile(f.path()).unwrap();
        assert!(!workers.contains_key("web"));
        assert!(workers.contains_key("wsgi"));
    }

    #[test]
    fn test_parse_procfile_missing_file() {
        let result = parse_procfile(Path::new("/nonexistent/Procfile"));
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_procfile_cron_valid() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "cron: */5 * * * * /usr/bin/task").unwrap();
        let workers = parse_procfile(f.path()).unwrap();
        assert!(workers.contains_key("cron"));
    }

    #[test]
    fn test_parse_procfile_cron_invalid_value() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "cron: 60 * * * * /usr/bin/task").unwrap();
        let workers = parse_procfile(f.path()).unwrap();
        assert!(!workers.contains_key("cron"));
    }

    #[test]
    fn test_parse_procfile_cron_rejects_hour_24() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "cron: 0 24 * * * /usr/bin/task").unwrap();
        let workers = parse_procfile(f.path()).unwrap();
        assert!(!workers.contains_key("cron"));
    }

    #[test]
    fn test_parse_procfile_cron_rejects_weekday_8() {
        // Standard Unix cron allows weekday 0-7 (0 and 7 both Sunday); 8 is
        // out of range and must be rejected by the shared engine.
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "cron: 0 0 * * 8 /usr/bin/task").unwrap();
        let workers = parse_procfile(f.path()).unwrap();
        assert!(!workers.contains_key("cron"));
    }

    #[test]
    fn test_parse_procfile_cron_accepts_weekday_7_sunday() {
        // 7 is a legal alias for Sunday in Unix cron — the old regex-based
        // validator wrongly rejected it; the engine accepts it.
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "cron: 0 0 * * 7 /usr/bin/task").unwrap();
        let workers = parse_procfile(f.path()).unwrap();
        assert!(workers.contains_key("cron"));
    }

    #[test]
    fn test_parse_procfile_cron_accepts_valid_bounds() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "cron: 59 23 31 12 6 /usr/bin/task").unwrap();
        let workers = parse_procfile(f.path()).unwrap();
        assert!(workers.contains_key("cron"));
    }

    #[test]
    fn test_parse_procfile_cron_accepts_ranges_and_lists() {
        // The old CRON_REGEXP rejected ranges/lists; the engine accepts them.
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "cron: 0 9 * * 1-5 /usr/bin/weekday-task").unwrap();
        writeln!(f, "cron2: 0,15,30,45 * * * * /usr/bin/quarter-hour").unwrap();
        let workers = parse_procfile(f.path()).unwrap();
        assert!(workers.contains_key("cron"));
        assert!(workers.contains_key("cron2"));
    }

    #[test]
    fn test_parse_procfile_empty_file() {
        let f = NamedTempFile::new().unwrap();
        let workers = parse_procfile(f.path()).unwrap();
        assert!(workers.is_empty());
    }
}
