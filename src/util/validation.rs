//! Input validation and sanitization utilities.
//!
//! # Security Model
//!
//! All user-supplied strings (app names, env var values, paths) must be validated
//! at the boundary before use. Functions here enforce character whitelists and
//! reject path-traversal sequences so that validated values are safe to use in
//! file-system operations and environment variable assignments.

use anyhow::Result;
use std::fs;
use std::path::Path;
use std::process;

use super::display::echo;

/// Verify that a resolved (canonicalized) path stays within an expected root directory.
/// Returns Ok(()) if the path is within bounds, Err otherwise.
///
/// Both `path` and `root` must exist on the filesystem; if either cannot be
/// canonicalized (e.g. because it does not exist or is a dangling symlink)
/// this function returns Err rather than falling back to the raw path.
/// Falling back would silently bypass the traversal check for non-existent
/// paths (an attacker could supply a path that only exists after the check).
pub fn ensure_path_within(path: &Path, root: &Path) -> Result<()> {
    let resolved = fs::canonicalize(path)
        .map_err(|e| anyhow::anyhow!("Cannot resolve path '{}': {}", path.display(), e))?;
    let root_resolved = fs::canonicalize(root)
        .map_err(|e| anyhow::anyhow!("Cannot resolve root '{}': {}", root.display(), e))?;
    if !resolved.starts_with(&root_resolved) {
        return Err(anyhow::anyhow!(
            "Path '{}' escapes expected root '{}'",
            resolved.display(),
            root_resolved.display()
        ));
    }
    Ok(())
}

/// Sanitize the app name: only allow alphanumeric, dots, underscores, hyphens.
/// Strip leading slashes, trim trailing whitespace.
/// Rejects path traversal attempts (`..`) and empty/dot-only names.
pub fn sanitize_app_name(app: &str) -> String {
    let stripped = app.trim_start_matches('/');
    let sanitized: String = stripped
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '.' || *c == '_' || *c == '-')
        .collect::<String>()
        .trim_end()
        .to_string();

    // Reject path traversal, empty names, and dot-only names (e.g. ".", "..")
    if sanitized.contains("..") || sanitized.is_empty() || sanitized.trim_matches('.').is_empty() {
        return String::new();
    }
    sanitized
}

/// Validate and sanitize app name, returning an error if invalid.
pub fn validate_app_name(app: &str) -> Result<String> {
    let sanitized = sanitize_app_name(app);
    if sanitized.is_empty() {
        return Err(anyhow::anyhow!(
            "Invalid app name '{}': contains invalid characters or path traversal sequences",
            app
        ));
    }
    Ok(sanitized)
}

/// Sanitize name, check app dir exists, exit(1) if not.
pub fn exit_if_invalid(app: &str, app_root: &Path) -> Result<String> {
    let app = sanitize_app_name(app);
    if app.is_empty() {
        echo(&format!("Error: invalid app name '{}'.", app), "red");
        echo(
            "App names must contain only alphanumeric characters, dots, underscores, and hyphens.",
            "yellow",
        );
        echo("Path traversal sequences (..) are not allowed.", "yellow");
        process::exit(1);
    }
    if !app_root.join(&app).exists() {
        echo(&format!("Error: app '{}' not found.", app), "red");
        echo("", "");
        echo("To deploy a new app:", "yellow");
        echo("  Option 1: Create app and push via git", "yellow");
        echo("    riku apps create myapp", "yellow");
        echo("    git remote add riku deploy@server:myapp", "yellow");
        echo("    git push riku main", "yellow");
        echo("", "");
        echo("  Option 2: Deploy from local folder", "yellow");
        echo("    riku deploy myapp --from ./path/to/app", "yellow");
        echo("", "");
        echo("Or list existing apps:", "yellow");
        echo("  riku apps", "yellow");
        process::exit(1);
    }
    Ok(app)
}

/// Convert a boolean-ish string to a boolean.
#[allow(dead_code)]
pub fn get_boolean(value: &str) -> bool {
    matches!(
        value.to_lowercase().as_str(),
        "1" | "on" | "true" | "enabled" | "yes" | "y"
    )
}

/// Validate and parse a positive integer environment variable.
/// Returns Ok(value) if valid, or Err with a helpful error message.
#[allow(dead_code)]
pub fn parse_positive_int(name: &str, value: &str) -> Result<u64, String> {
    value
        .parse::<u64>()
        .map_err(|_| {
            format!(
                "Invalid value for {}: '{}' is not a valid positive integer",
                name, value
            )
        })
        .and_then(|v| {
            if v == 0 {
                Err(format!(
                    "Invalid value for {}: must be greater than 0",
                    name
                ))
            } else {
                Ok(v)
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_sanitize_valid_name() {
        assert_eq!(sanitize_app_name("my-app"), "my-app");
        assert_eq!(sanitize_app_name("my_app.v2"), "my_app.v2");
        assert_eq!(sanitize_app_name("app123"), "app123");
    }

    #[test]
    fn test_sanitize_invalid_chars() {
        assert_eq!(sanitize_app_name("my app!@#"), "myapp");
        assert_eq!(sanitize_app_name("app/name"), "appname");
        assert_eq!(sanitize_app_name("a b c"), "abc");
    }

    #[test]
    fn test_sanitize_leading_slashes() {
        assert_eq!(sanitize_app_name("/my-app"), "my-app");
        assert_eq!(sanitize_app_name("///app"), "app");
        assert_eq!(sanitize_app_name("/"), "");
    }

    #[test]
    fn test_sanitize_trailing_whitespace() {
        assert_eq!(sanitize_app_name("my-app  "), "my-app");
        assert_eq!(sanitize_app_name("app\t"), "app");
    }

    #[test]
    fn test_sanitize_path_traversal() {
        assert_eq!(sanitize_app_name(".."), "");
        assert_eq!(sanitize_app_name("../etc/passwd"), "");
        assert_eq!(sanitize_app_name("app/../secret"), "");
        assert_eq!(sanitize_app_name("my..app"), "");
        assert_eq!(sanitize_app_name("..."), "");
    }

    #[test]
    fn test_sanitize_dot_only_names() {
        assert_eq!(sanitize_app_name("."), "");
        assert_eq!(sanitize_app_name("..."), "");
    }

    #[test]
    fn test_sanitize_valid_dotted_names() {
        assert_eq!(sanitize_app_name("my-app.v2"), "my-app.v2");
        assert_eq!(sanitize_app_name(".hidden-app"), ".hidden-app");
    }

    #[test]
    fn test_ensure_path_within_accepts_child() {
        let temp_dir = TempDir::new().unwrap();
        let child = temp_dir.path().join("subdir");
        fs::create_dir(&child).unwrap();
        assert!(ensure_path_within(&child, temp_dir.path()).is_ok());
    }

    #[test]
    fn test_ensure_path_within_rejects_outside() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path().join("root");
        fs::create_dir(&root).unwrap();
        assert!(ensure_path_within(temp_dir.path(), &root).is_err());
    }

    #[test]
    fn test_get_boolean_truthy() {
        for val in &["1", "on", "true", "enabled", "yes", "y"] {
            assert!(get_boolean(val), "expected true for '{}'", val);
        }
    }

    #[test]
    fn test_get_boolean_case_insensitive() {
        assert!(get_boolean("True"));
        assert!(get_boolean("TRUE"));
        assert!(get_boolean("ON"));
        assert!(get_boolean("Yes"));
        assert!(get_boolean("Y"));
        assert!(get_boolean("Enabled"));
    }

    #[test]
    fn test_get_boolean_falsy() {
        assert!(!get_boolean("0"));
        assert!(!get_boolean("off"));
        assert!(!get_boolean("false"));
        assert!(!get_boolean("no"));
        assert!(!get_boolean("n"));
        assert!(!get_boolean(""));
        assert!(!get_boolean("random"));
    }

    #[test]
    fn test_parse_positive_int_valid() {
        assert_eq!(parse_positive_int("TEST", "100"), Ok(100));
        assert_eq!(parse_positive_int("TEST", "1"), Ok(1));
        assert_eq!(parse_positive_int("TEST", "999999"), Ok(999999));
    }

    #[test]
    fn test_parse_positive_int_invalid() {
        assert!(parse_positive_int("TEST", "abc").is_err());
        assert!(parse_positive_int("TEST", "-5").is_err());
        assert!(parse_positive_int("TEST", "0").is_err());
        assert!(parse_positive_int("TEST", "").is_err());
    }
}
