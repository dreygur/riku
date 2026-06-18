//! Git receive-pack and upload-pack command handlers.

use anyhow::Result;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

use crate::config::RikuPaths;
use crate::util::echo;

use super::repo::{ensure_repo_symlink, POST_RECEIVE_HOOK};

/// Handle git pushes for an app. Sets up bare repo and hook if needed,
/// then delegates to git-receive-pack directly.
pub fn cmd_git_receive_pack(paths: &RikuPaths, app: &str) -> Result<()> {
    let app = crate::util::validate_app_name(app)?;

    // Ensure symlink is set up for user's bare repo
    ensure_repo_symlink(paths, &app)?;

    let riku_repo = paths.git_root.join(&app);

    // Check for an actual bare-repo marker (HEAD), not just the directory's
    // existence. `git init --bare` always creates the target directory
    // itself, so checking `riku_repo.exists()` after first creating the
    // hooks/ subdirectory would always be true and `git init --bare` would
    // never run, leaving a directory with a hooks/ folder but no git
    // internals (fatal: '<path>' does not appear to be a git repository).
    if !riku_repo.join("HEAD").exists() {
        fs::create_dir_all(&paths.git_root)?;
        let status = Command::new("git")
            .arg("init")
            .arg("--quiet")
            .arg("--bare")
            .arg(&app)
            .current_dir(&paths.git_root)
            .status()?;
        if !status.success() {
            echo("Error: git init failed.", "red");
        }
    }

    let hook_path = riku_repo.join("hooks").join("post-receive");
    if !hook_path.exists() {
        if let Some(parent) = hook_path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(&hook_path, POST_RECEIVE_HOOK)?;

        // Make hook executable
        let meta = fs::metadata(&hook_path)?;
        let mode = meta.permissions().mode();
        fs::set_permissions(&hook_path, fs::Permissions::from_mode(mode | 0o755))?;
    }

    // Delegate directly to git-receive-pack (avoiding shell interpolation)
    let status = Command::new("git-receive-pack")
        .arg(paths.git_root.join(&app))
        .status()?;

    std::process::exit(status.code().unwrap_or(1));
}

/// Handle git upload-pack for an app.
pub fn cmd_git_upload_pack(paths: &RikuPaths, app: &str) -> Result<()> {
    let app = crate::util::validate_app_name(app)?;

    // Call git-upload-pack directly (avoiding shell interpolation)
    let status = Command::new("git-upload-pack")
        .arg(paths.git_root.join(&app))
        .status()?;

    std::process::exit(status.code().unwrap_or(1));
}
