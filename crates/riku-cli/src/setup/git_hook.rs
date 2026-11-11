//! Global git post-receive hook creation.

use anyhow::{anyhow, Result};
use std::fs;
use std::os::unix::fs::PermissionsExt;

use crate::config::RikuPaths;
use crate::git::repo::post_receive_hook;

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
    if let Err(e) = fs::write(&post_receive, post_receive_hook()) {
        return Err(anyhow!("failed to write {}: {e}", post_receive.display()));
    }
    if let Err(e) = fs::set_permissions(&post_receive, fs::Permissions::from_mode(0o755)) {
        return Err(anyhow!("failed to chmod {}: {e}", post_receive.display()));
    }
    Ok(())
}

#[cfg(test)]
#[path = "git_hook_tests.rs"]
mod tests;
