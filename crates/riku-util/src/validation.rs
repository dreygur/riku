//! Input validation and sanitization utilities.
//!
//! # Security Model
//!
//! All user-supplied strings (app names, env var values, paths) must be validated
//! at the boundary before use. Functions here enforce character whitelists and
//! reject path-traversal sequences so that validated values are safe to use in
//! file-system operations and environment variable assignments.

use anyhow::{anyhow, Result};
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

/// Ending of a bare git repository's directory. A push to `<app>.git` deploys
/// `<app>` and keeps its repository at `repos/<app>.git`, so no app name may
/// end with it.
pub const REPO_SUFFIX: &str = ".git";

fn allowed_chars(app: &str) -> String {
    app.trim_start_matches('/')
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '.' || *c == '_' || *c == '-')
        .collect::<String>()
        .trim_end()
        .to_string()
}

/// Sanitize the app name: only allow alphanumeric, dots, underscores, hyphens.
/// Strip leading slashes, trim trailing whitespace.
/// Rejects path traversal attempts (`..`), empty and dot-only names, and
/// names ending in [`REPO_SUFFIX`].
pub fn sanitize_app_name(app: &str) -> String {
    let sanitized = allowed_chars(app);
    if sanitized.contains("..")
        || sanitized.trim_matches('.').is_empty()
        || sanitized.ends_with(REPO_SUFFIX)
    {
        return String::new();
    }
    sanitized
}

/// Validate and sanitize app name, returning an error if invalid.
pub fn validate_app_name(app: &str) -> Result<String> {
    if allowed_chars(app).ends_with(REPO_SUFFIX) {
        return Err(anyhow::anyhow!(
            "Invalid app name '{app}': names ending in {REPO_SUFFIX} are reserved, because a push to <app>{REPO_SUFFIX} deploys <app>"
        ));
    }
    let sanitized = sanitize_app_name(app);
    if sanitized.is_empty() {
        return Err(anyhow::anyhow!(
            "Invalid app name '{}': contains invalid characters or path traversal sequences",
            app
        ));
    }
    Ok(sanitized)
}

/// The sanitized name of an app that exists under `app_root`, or an error.
/// It prints nothing and never exits, so a caller that answers in JSON (the
/// agent commands) keeps control of its output.
pub fn existing_app(app: &str, app_root: &Path) -> Result<String> {
    let name = sanitize_app_name(app);
    if name.is_empty() {
        return Err(anyhow!("invalid app name '{app}'"));
    }
    if !app_root.join(&name).exists() {
        return Err(anyhow!("app '{name}' not found"));
    }
    Ok(name)
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
        echo(
            "Path traversal sequences (..) and a trailing .git are not allowed.",
            "yellow",
        );
        process::exit(1);
    }
    if !app_root.join(&app).exists() {
        echo(&format!("Error: app '{}' not found.", app), "red");
        echo("", "");
        echo("To deploy a new app:", "yellow");
        echo("  Option 1: Create app and push via git", "yellow");
        echo("    riku apps create myapp", "yellow");
        echo("    git remote add riku riku@server:myapp", "yellow");
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

#[cfg(test)]
#[path = "validation_tests.rs"]
mod tests;
