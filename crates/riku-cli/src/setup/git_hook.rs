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
    // See the matching comment in apps/create.rs: `riku git-hook` reads the
    // "oldrev newrev refname" lines directly from its own stdin, so this
    // hook must not `read` them itself first (that would starve the child
    // of input, making it silently deploy nothing on the common single-ref
    // push).
    let hook_script = r#"#!/bin/bash
# Riku global post-receive hook
# This hook is called when code is pushed to any app repository

APP=$(basename "$(pwd)" .git)
REPO_PATH="$(pwd)"
RIKU_BIN="$HOME/.local/bin/riku"
if [ -x "$RIKU_BIN" ]; then
    exec "$RIKU_BIN" git-hook "$APP" "$REPO_PATH"
else
    echo " !     Riku binary not found at $RIKU_BIN" >&2
    exit 1
fi
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
