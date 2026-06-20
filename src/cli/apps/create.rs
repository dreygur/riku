use anyhow::Result;
use std::fs;
use std::process::Command;

use crate::config::RikuPaths;
use crate::util::echo;

/// Create a new application (directory and git repository).
///
/// Returns the sanitized app name actually used on disk, which may differ
/// from `name` (see `validate_app_name`) — callers must use this value for
/// any follow-up lookups instead of echoing the raw input back.
pub fn cmd_apps_create(paths: &RikuPaths, name: &str) -> Result<String> {
    use std::os::unix::fs::PermissionsExt;

    let app = crate::util::validate_app_name(name)?;

    if paths.app_root.join(&app).exists() {
        anyhow::bail!("app '{}' already exists", app);
    }

    // Create app directory
    let app_dir = paths.app_root.join(&app);
    fs::create_dir_all(&app_dir)?;
    echo(
        &format!("✓ Created app directory: {}", app_dir.display()),
        "green",
    );

    // Create git repository
    let repo_dir = paths.git_root.join(format!("{}.git", app));
    fs::create_dir_all(&repo_dir)?;

    Command::new("git")
        .args(["init", "--bare"])
        .current_dir(&repo_dir)
        .output()?;

    echo(
        &format!("✓ Created git repository: {}", repo_dir.display()),
        "green",
    );

    // Create post-receive hook
    let hooks_dir = repo_dir.join("hooks");
    fs::create_dir_all(&hooks_dir)?;

    let post_receive = hooks_dir.join("post-receive");
    let hook_script = format!(
        r#"#!/bin/bash
# Riku post-receive hook for app: {}

while read oldrev newrev refname; do
    RIKU_BIN="$HOME/.local/bin/riku"
    if [ -x "$RIKU_BIN" ]; then
        # Get the actual repo path
        REPO_PATH="$(pwd)"
        "$RIKU_BIN" git-hook "{}" "$REPO_PATH"
    else
        echo " !     Riku binary not found at $RIKU_BIN"
    fi
done
"#,
        app, app
    );

    fs::write(&post_receive, hook_script)?;
    fs::set_permissions(&post_receive, PermissionsExt::from_mode(0o755))?;

    echo(
        &format!("✓ Created git hook: {}", post_receive.display()),
        "green",
    );
    echo("", "");

    echo(&format!("App '{}' created successfully!", app), "green");
    echo("", "");
    echo("Deploy your code:", "yellow");
    echo(
        &format!("  git remote add riku deploy@your-server:{}", app),
        "yellow",
    );
    echo("  git push riku main", "yellow");
    echo("", "");

    Ok(app)
}
