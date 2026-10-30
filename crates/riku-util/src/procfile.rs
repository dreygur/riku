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
fn is_valid_cron_command(command: &str) -> bool {
    let fields: Vec<&str> = command.split_whitespace().collect();
    if fields.len() < 5 {
        return false;
    }
    let schedule = fields[..5].join(" ");
    validate_cron_expression(&schedule)
}

/// A process kind names worker config and log files, so it is limited to
/// letters, digits, `_` and `-`.
fn is_valid_kind(kind: &str) -> bool {
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
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_parse_procfile_basic() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "web: python app.py").unwrap();
        writeln!(f, "worker: celery -A tasks").unwrap();
        let workers = parse_procfile(f.path()).unwrap().unwrap();
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
        let workers = parse_procfile(f.path()).unwrap().unwrap();
        assert_eq!(workers.len(), 1);
        assert_eq!(workers.get("web").unwrap(), "python app.py");
    }

    #[test]
    fn test_parse_procfile_wsgi_trumps_web() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "web: python app.py").unwrap();
        writeln!(f, "wsgi: app:application").unwrap();
        let workers = parse_procfile(f.path()).unwrap().unwrap();
        assert!(!workers.contains_key("web"));
        assert!(workers.contains_key("wsgi"));
    }

    #[test]
    fn test_parse_procfile_missing_file() {
        let result = parse_procfile(Path::new("/nonexistent/Procfile")).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_procfile_cron_valid() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "cron: */5 * * * * /usr/bin/task").unwrap();
        let workers = parse_procfile(f.path()).unwrap().unwrap();
        assert!(workers.contains_key("cron"));
    }

    #[test]
    fn test_parse_procfile_cron_invalid_value() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "cron: 60 * * * * /usr/bin/task").unwrap();
        assert!(
            parse_procfile(f.path()).is_err(),
            "a bad schedule must fail the parse"
        );
    }

    #[test]
    fn test_parse_procfile_cron_rejects_hour_24() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "cron: 0 24 * * * /usr/bin/task").unwrap();
        assert!(
            parse_procfile(f.path()).is_err(),
            "a bad schedule must fail the parse"
        );
    }

    #[test]
    fn test_parse_procfile_cron_rejects_weekday_8() {
        // Standard Unix cron allows weekday 0-7 (0 and 7 both Sunday); 8 is
        // out of range and must be rejected by the shared engine.
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "cron: 0 0 * * 8 /usr/bin/task").unwrap();
        assert!(
            parse_procfile(f.path()).is_err(),
            "a bad schedule must fail the parse"
        );
    }

    #[test]
    fn test_parse_procfile_cron_accepts_weekday_7_sunday() {
        // 7 is a legal alias for Sunday in Unix cron: the old regex-based
        // validator wrongly rejected it; the engine accepts it.
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "cron: 0 0 * * 7 /usr/bin/task").unwrap();
        let workers = parse_procfile(f.path()).unwrap().unwrap();
        assert!(workers.contains_key("cron"));
    }

    #[test]
    fn test_parse_procfile_cron_accepts_valid_bounds() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "cron: 59 23 31 12 6 /usr/bin/task").unwrap();
        let workers = parse_procfile(f.path()).unwrap().unwrap();
        assert!(workers.contains_key("cron"));
    }

    #[test]
    fn test_parse_procfile_cron_accepts_ranges_and_lists() {
        // The old CRON_REGEXP rejected ranges/lists; the engine accepts them.
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "cron: 0 9 * * 1-5 /usr/bin/weekday-task").unwrap();
        writeln!(f, "cron2: 0,15,30,45 * * * * /usr/bin/quarter-hour").unwrap();
        let workers = parse_procfile(f.path()).unwrap().unwrap();
        assert!(workers.contains_key("cron"));
        assert!(workers.contains_key("cron2"));
    }

    #[test]
    fn whitespace_and_comments_only_mean_no_processes() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "   \n\t\n# nothing here").unwrap();
        let workers = parse_procfile(f.path()).unwrap().unwrap();
        assert!(workers.is_empty(), "got {workers:?}");
    }

    #[test]
    fn rejects_a_line_that_is_not_kind_colon_command() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "web: python app.py").unwrap();
        writeln!(f, "worker python worker.py").unwrap();
        let err = parse_procfile(f.path()).unwrap_err();
        assert!(err.to_string().contains("line 2"), "{err}");
    }

    #[test]
    fn rejects_a_process_kind_that_is_not_a_plain_name() {
        for line in ["../x: rm -rf ~", "foo bar: baz", ": python app.py"] {
            let mut f = NamedTempFile::new().unwrap();
            writeln!(f, "{line}").unwrap();
            assert!(
                parse_procfile(f.path()).is_err(),
                "{line:?} should be rejected"
            );
        }
    }

    #[test]
    fn test_parse_procfile_empty_file() {
        let f = NamedTempFile::new().unwrap();
        let workers = parse_procfile(f.path()).unwrap().unwrap();
        assert!(workers.is_empty());
    }
}
