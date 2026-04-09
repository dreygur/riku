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

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use tempfile::TempDir;

    /// Initialize a bare git repo and a working clone, commit one file, and return
    /// both temp directories and the HEAD sha.
    fn make_git_repo() -> (TempDir, TempDir, String) {
        let bare = TempDir::new().unwrap();
        let work = TempDir::new().unwrap();

        // init bare repo
        Command::new("git")
            .args(["init", "--bare", bare.path().to_str().unwrap()])
            .output()
            .unwrap();

        // clone into work tree
        Command::new("git")
            .args([
                "clone",
                bare.path().to_str().unwrap(),
                work.path().to_str().unwrap(),
            ])
            .output()
            .unwrap();

        // configure identity so commits work in CI
        Command::new("git")
            .args([
                "-C",
                work.path().to_str().unwrap(),
                "config",
                "user.email",
                "test@example.com",
            ])
            .output()
            .unwrap();
        Command::new("git")
            .args([
                "-C",
                work.path().to_str().unwrap(),
                "config",
                "user.name",
                "Test",
            ])
            .output()
            .unwrap();

        // create a commit
        std::fs::write(work.path().join("README"), "hello").unwrap();
        Command::new("git")
            .args(["-C", work.path().to_str().unwrap(), "add", "."])
            .output()
            .unwrap();
        Command::new("git")
            .args(["-C", work.path().to_str().unwrap(), "commit", "-m", "init"])
            .output()
            .unwrap();
        Command::new("git")
            .args([
                "-C",
                work.path().to_str().unwrap(),
                "push",
                "origin",
                "HEAD",
            ])
            .output()
            .unwrap();

        // get HEAD sha
        let sha_out = Command::new("git")
            .args(["-C", work.path().to_str().unwrap(), "rev-parse", "HEAD"])
            .output()
            .unwrap();
        let sha = String::from_utf8(sha_out.stdout)
            .unwrap()
            .trim()
            .to_string();

        (bare, work, sha)
    }

    #[test]
    fn test_git_reset_to_valid_rev_succeeds() -> Result<()> {
        let (_bare, work, sha) = make_git_repo();
        git_reset(work.path(), &sha)
    }

    #[test]
    fn test_git_reset_to_invalid_rev_fails() {
        let (_bare, work, _sha) = make_git_repo();
        let result = git_reset(work.path(), "deadbeefdeadbeef");
        assert!(result.is_err(), "Expected error for invalid rev");
    }

    #[test]
    fn test_git_fetch_best_effort_does_not_panic() {
        let (_bare, work, _sha) = make_git_repo();
        // git_fetch is best-effort; calling it on a valid repo must not panic
        git_fetch(work.path());
    }

    #[test]
    fn test_sync_app_repo_without_newrev() -> Result<()> {
        let (_bare, work, _sha) = make_git_repo();
        // None for newrev: only fetches (best-effort), no reset
        sync_app_repo(work.path(), None)
    }

    #[test]
    fn test_sync_app_repo_with_valid_newrev() -> Result<()> {
        let (_bare, work, sha) = make_git_repo();
        sync_app_repo(work.path(), Some(&sha))
    }

    #[test]
    fn test_sync_app_repo_with_invalid_newrev_returns_error() {
        let (_bare, work, _sha) = make_git_repo();
        let result = sync_app_repo(
            work.path(),
            Some("0000000000000000000000000000000000000000"),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_git_update_submodules_no_submodules() {
        // Repo with no submodules: should complete without panic
        let (_bare, work, _sha) = make_git_repo();
        git_update_submodules(work.path());
    }
}
