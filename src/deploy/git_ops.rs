//! Git operations for deployment: fetch, reset, and submodule management.

use anyhow::Result;
use std::path::Path;
use std::process::Command;

use crate::util::echo;

/// Fetch from origin (best-effort; logs a warning on failure).
pub fn git_fetch(app_path: &Path) {
    let result = Command::new("git")
        .args(["fetch", "--quiet", "origin"])
        .current_dir(app_path)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .status();

    if let Err(e) = result {
        echo(&format!("Warning: git fetch failed: {}", e), "yellow");
    }
}

/// Hard-reset the working tree to `newrev`.
///
/// Returns an error if the reset command fails or exits non-zero.
pub fn git_reset(app_path: &Path, newrev: &str) -> Result<()> {
    let status = Command::new("git")
        .args(["reset", "--hard", newrev])
        .current_dir(app_path)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .status()
        .map_err(|e| anyhow::anyhow!("Failed to run git reset: {}", e))?;

    if !status.success() {
        return Err(anyhow::anyhow!(
            "git reset --hard {} failed (exit {}). Deploy aborted.",
            newrev,
            status.code().unwrap_or(-1)
        ));
    }
    Ok(())
}

/// Init and update git submodules (best-effort; logs warnings on failure).
pub fn git_update_submodules(app_path: &Path) {
    let init = Command::new("git")
        .args(["submodule", "init"])
        .current_dir(app_path)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .status();

    if let Err(e) = init {
        echo(
            &format!("Warning: git submodule init failed: {}", e),
            "yellow",
        );
    }

    let update = Command::new("git")
        .args(["submodule", "update", "--recursive"])
        .current_dir(app_path)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .status();

    if let Err(e) = update {
        echo(
            &format!("Warning: git submodule update failed: {}", e),
            "yellow",
        );
    }
}

/// Fetch latest changes and optionally hard-reset to `newrev`.
///
/// When `newrev` is `Some`, the working tree is reset and submodules are updated.
pub fn sync_app_repo(app_path: &Path, newrev: Option<&str>) -> Result<()> {
    git_fetch(app_path);

    if let Some(rev) = newrev {
        git_reset(app_path, rev)?;
        git_update_submodules(app_path);
    }

    Ok(())
}
