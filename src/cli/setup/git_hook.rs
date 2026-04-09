//! Global git post-receive hook creation.

use anyhow::Result;
use std::fs;

use crate::config::RikuPaths;
use crate::util::echo;

/// Write the global post-receive hook script to ~/.riku/../hooks/post-receive.
pub fn create_git_hook(paths: &RikuPaths) -> Result<()> {
    let hooks_dir = paths
        .git_root
        .parent()
        .ok_or_else(|| anyhow::anyhow!("git_root has no parent directory"))?
        .join("hooks");

    if !hooks_dir.exists() {
        fs::create_dir_all(&hooks_dir)?;
    }

    let post_receive = hooks_dir.join("post-receive");
    let hook_script = r#"#!/bin/bash
# Riku global post-receive hook
# This hook is called when code is pushed to any app repository

while read oldrev newrev refname; do
    # Extract app name from repository path
    APP=$(basename "$(pwd)" .git)
    # Get the actual repo path
    REPO_PATH="$(pwd)"

    # Run riku git-hook
    RIKU_BIN="$HOME/.local/bin/riku"
    if [ -x "$RIKU_BIN" ]; then
        "$RIKU_BIN" git-hook "$APP" "$REPO_PATH"
    else
        echo " !     Riku binary not found at $RIKU_BIN"
    fi
done
"#;

    fs::write(&post_receive, hook_script)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&post_receive, fs::Permissions::from_mode(0o755))?;
    }

    echo("      ✓ Global git hook created", "green");

    Ok(())
}
