use anyhow::{anyhow, Result};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

use crate::config::RikuPaths;
use crate::util::{echo, ROOT_BINARY};

/// The per-app post-receive hook. `riku git-hook` reads the "oldrev newrev
/// refname" lines git feeds the hook directly from its own stdin (one
/// iteration per updated ref), so the hook must not `read` them first: that
/// would leave `riku git-hook` an empty stdin and a silent no-op deploy for
/// a single-ref push.
pub(crate) fn app_hook_script(app: &str) -> String {
    format!(
        r#"#!/bin/bash
# Riku post-receive hook for app: {app}
BINARY="{ROOT_BINARY}"
if [ -x "$BINARY" ]; then
    REPO_PATH="$(pwd)"
    exec "$BINARY" git-hook "{app}" "$REPO_PATH"
else
    echo " !     Riku binary not found at $BINARY" >&2
    exit 1
fi
"#
    )
}

fn create_dir(dir: &Path) -> Result<()> {
    match fs::create_dir_all(dir) {
        Ok(()) => Ok(()),
        Err(e) => Err(anyhow!("failed to create {}: {e}", dir.display())),
    }
}

fn init_bare_repo(repo_dir: &Path) -> Result<()> {
    match Command::new("git")
        .args(["init", "--bare"])
        .current_dir(repo_dir)
        .output()
    {
        Ok(_) => Ok(()),
        Err(e) => Err(anyhow!("failed to run git init: {e}")),
    }
}

fn write_hook(hook: &Path, app: &str) -> Result<()> {
    if let Err(e) = fs::write(hook, app_hook_script(app)) {
        return Err(anyhow!("failed to write {}: {e}", hook.display()));
    }
    match fs::set_permissions(hook, fs::Permissions::from_mode(0o755)) {
        Ok(()) => Ok(()),
        Err(e) => Err(anyhow!("failed to chmod {}: {e}", hook.display())),
    }
}

/// Create a new application (directory and git repository).
#[allow(clippy::question_mark)]
pub fn cmd_apps_create(paths: &RikuPaths, name: &str) -> Result<()> {
    let app = match crate::util::validate_app_name(name) {
        Ok(app) => app,
        Err(e) => return Err(anyhow!("invalid app name: {e}")),
    };

    if paths.app_root.join(&app).exists() {
        anyhow::bail!("app '{}' already exists", app);
    }

    let app_dir = paths.app_root.join(&app);
    if let Err(e) = create_dir(&app_dir) {
        return Err(e);
    }
    echo(
        &format!("✓ Created app directory: {}", app_dir.display()),
        "green",
    );

    let repo_dir = paths.git_root.join(format!("{}.git", app));
    if let Err(e) = create_dir(&repo_dir) {
        return Err(e);
    }
    if let Err(e) = init_bare_repo(&repo_dir) {
        return Err(e);
    }
    echo(
        &format!("✓ Created git repository: {}", repo_dir.display()),
        "green",
    );

    let hooks_dir = repo_dir.join("hooks");
    if let Err(e) = create_dir(&hooks_dir) {
        return Err(e);
    }
    let post_receive = hooks_dir.join("post-receive");
    if let Err(e) = write_hook(&post_receive, &app) {
        return Err(e);
    }
    echo(
        &format!("✓ Created git hook: {}", post_receive.display()),
        "green",
    );
    echo("", "");

    echo(&format!("App '{}' created successfully!", app), "green");
    echo("", "");
    echo("Deploy your code:", "yellow");
    echo(
        &format!("  git remote add riku riku@your-server:{}", app),
        "yellow",
    );
    echo("  git push riku main", "yellow");
    echo("", "");

    Ok(())
}
