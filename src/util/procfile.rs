//! Procfile parsing utilities.
//!
//! Parses Heroku-style Procfile entries, validates cron expressions,
//! and enforces the rule that WSGI workers supersede plain web workers.

use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use super::display::echo;

/// Cron regexp matching five time fields followed by a command.
const CRON_REGEXP: &str = r"^((?:(?:\*/)?\d+)|\*) ((?:(?:\*/)?\d+)|\*) ((?:(?:\*/)?\d+)|\*) ((?:(?:\*/)?\d+)|\*) ((?:(?:\*/)?\d+)|\*) (.*)$";

/// Pre-compiled cron regex for performance.
pub(crate) static CRON_RE: Lazy<Regex> = Lazy::new(|| Regex::new(CRON_REGEXP).unwrap());

/// Pre-compiled environment variable expansion regex (also used by `env.rs`).
pub(crate) static ENVVAR_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\$(\w+|\{([^}]*)\})").unwrap());

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

            // Check for cron patterns
            if kind.starts_with("cron") {
                // Cron field upper bounds: minute(0-59), hour(0-23), day(1-31), month(1-12), weekday(0-6)
                let limits = [59, 23, 31, 12, 6];
                if let Some(caps) = CRON_RE.captures(&command) {
                    let mut valid = true;
                    for i in 0..limits.len() {
                        let field = &caps[i + 1];
                        let num_str = field.replace("*/", "").replace('*', "1");
                        match num_str.parse::<u32>() {
                            Ok(n) if n > limits[i] => {
                                valid = false;
                                break;
                            }
                            Err(_) => {
                                valid = false;
                                break;
                            }
                            _ => {}
                        }
                    }
                    if !valid {
                        echo(
                            &format!(
                                "Warning: misformatted Procfile entry '{}' at line {}",
                                line, line_number
                            ),
                            "yellow",
                        );
                        continue;
                    }
                } else {
                    echo(
                        &format!(
                            "Warning: misformatted Procfile entry '{}' at line {}",
                            line, line_number
                        ),
                        "yellow",
                    );
                    continue;
                }
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
        writeln!(f, "").unwrap();
        writeln!(f, "web: python app.py").unwrap();
        writeln!(f, "").unwrap();
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
    fn test_parse_procfile_cron_rejects_weekday_7() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "cron: 0 0 * * 7 /usr/bin/task").unwrap();
        let workers = parse_procfile(f.path()).unwrap();
        assert!(!workers.contains_key("cron"));
    }

    #[test]
    fn test_parse_procfile_cron_accepts_valid_bounds() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "cron: 59 23 31 12 6 /usr/bin/task").unwrap();
        let workers = parse_procfile(f.path()).unwrap();
        assert!(workers.contains_key("cron"));
    }

    #[test]
    fn test_parse_procfile_empty_file() {
        let f = NamedTempFile::new().unwrap();
        let workers = parse_procfile(f.path()).unwrap();
        assert!(workers.is_empty());
    }
}
