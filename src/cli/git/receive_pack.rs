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

    let hook_path = paths.git_root.join(&app).join("hooks").join("post-receive");

    if !hook_path.exists() {
        // Create hooks directory
        if let Some(parent) = hook_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Initialize bare repo at riku's location if it doesn't exist
        let riku_repo = paths.git_root.join(&app);
        if !riku_repo.exists() {
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
