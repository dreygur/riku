//! Global git post-receive hook creation.

use anyhow::{anyhow, Result};
use std::fs;
use std::os::unix::fs::PermissionsExt;

use crate::config::RikuPaths;
use crate::util::echo;

/// `riku git-hook` reads the "oldrev newrev refname" lines directly from its
/// own stdin, so this hook must not `read` them itself first: that would
/// starve the child of input and silently deploy nothing on a single-ref
/// push.
pub(crate) const GLOBAL_HOOK: &str = r#"#!/bin/bash
# Riku global post-receive hook
# This hook is called when code is pushed to any app repository

APP=$(basename "$(pwd)" .git)
REPO_PATH="$(pwd)"
BINARY="/usr/local/bin/riku"
if [ -x "$BINARY" ]; then
    exec "$BINARY" git-hook "$APP" "$REPO_PATH"
else
    echo " !     Riku binary not found at $BINARY" >&2
    exit 1
fi
"#;

/// Write the global post-receive hook script to ~/.riku/hooks/post-receive.
pub fn create_git_hook(paths: &RikuPaths) -> Result<()> {
    let hooks_dir = match paths.git_root.parent() {
        Some(parent) => parent.join("hooks"),
        None => return Err(anyhow!("git_root has no parent directory")),
    };
    if let Err(e) = fs::create_dir_all(&hooks_dir) {
        return Err(anyhow!("failed to create {}: {e}", hooks_dir.display()));
    }

    let post_receive = hooks_dir.join("post-receive");
    if let Err(e) = fs::write(&post_receive, GLOBAL_HOOK) {
        return Err(anyhow!("failed to write {}: {e}", post_receive.display()));
    }
    if let Err(e) = fs::set_permissions(&post_receive, fs::Permissions::from_mode(0o755)) {
        return Err(anyhow!("failed to chmod {}: {e}", post_receive.display()));
    }

    echo("      ✓ Global git hook created", "green");
    Ok(())
}

#[cfg(test)]
#[path = "git_hook_tests.rs"]
mod tests;
