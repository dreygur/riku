use anyhow::{bail, Result};
use std::fs;
use std::io::{self, BufRead};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

use crate::config::RikuPaths;
use crate::util::{echo, sanitize_app_name};

/// Create symlink from user's bare repo to riku's repos directory.
/// If bare repo exists at ~/app.git, symlink ~/.riku/repos/app.git → ~/app.git
/// Otherwise use ~/.riku/repos/app.git as the canonical location.
pub fn ensure_repo_symlink(paths: &RikuPaths, app: &str) -> Result<()> {
    let home = std::env::var("HOME").unwrap_or_else(|_| String::from("."));
    let user_repo = Path::new(&home).join(format!("{}.git", app));
    let riku_repo = paths.git_root.join(format!("{}.git", app));

    // If user's bare repo exists, create symlink
    if user_repo.exists() {
        if riku_repo.exists() {
            // Check if it's already a symlink to the right place
            if let Ok(target) = fs::read_link(&riku_repo) {
                if target == user_repo {
                    return Ok(()); // Already correctly symlinked
                }
            }
            // Remove existing file/symlink
            fs::remove_file(&riku_repo)?;
        }
        // Create symlink: ~/.riku/repos/app.git → ~/app.git
        std::os::unix::fs::symlink(&user_repo, &riku_repo)?;
        echo(
            &format!(
                "Symlinked {} → {}",
                riku_repo.display(),
                user_repo.display()
            ),
            "green",
        );
    }
    // If user repo doesn't exist, riku will create it at ~/.riku/repos/app.git

    Ok(())
}

/// Set up post-receive hook in a bare repo for auto-deploy.
pub fn setup_post_receive_hook(repo_path: &Path, _app: &str) -> Result<()> {
    let hooks_dir = repo_path.join("hooks");
    fs::create_dir_all(&hooks_dir)?;

    let hook_path = hooks_dir.join("post-receive");
    let hook_content = r#"#!/usr/bin/env bash
set -e; set -o pipefail;
# Find riku binary from PATH or use common locations
RIKU_BIN="${RIKU_BIN:-$(command -v riku)}"
if [ -z "$RIKU_BIN" ]; then
    # Fallback to common installation paths
    if [ -x "$HOME/.local/bin/riku" ]; then
        RIKU_BIN="$HOME/.local/bin/riku"
    elif [ -x "$HOME/riku" ]; then
        RIKU_BIN="$HOME/riku"
    elif [ -x "/usr/local/bin/riku" ]; then
        RIKU_BIN="/usr/local/bin/riku"
    else
        echo "Error: riku binary not found" >&2
        exit 1
    fi
fi
cat | PIKU_ROOT="${PIKU_ROOT:-$HOME/.riku}" "$RIKU_BIN" git-hook "$2"
"#;

    fs::write(&hook_path, hook_content)?;
    fs::set_permissions(&hook_path, PermissionsExt::from_mode(0o755))?;

    echo(
        &format!("✓ Created post-receive hook in {}", repo_path.display()),
        "green",
    );
    Ok(())
}

/// Extract files from bare repo to app directory using git archive.
pub fn extract_bare_repo_to_app(bare_repo: &Path, app: &str, paths: &RikuPaths) -> Result<()> {
    let app_dir = paths.app_root.join(app);

    // Create app directory
    if app_dir.exists() {
        fs::remove_dir_all(&app_dir)?;
    }
    fs::create_dir_all(&app_dir)?;

    // Use git archive piped to tar to extract files
    let status = Command::new("sh")
        .args([
            "-c",
            &format!(
                "git archive --format=tar HEAD | tar -xf - -C '{}'",
                app_dir.display()
            ),
        ])
        .current_dir(bare_repo)
        .status()?;

    if !status.success() {
        bail!("Failed to extract files from bare repo");
    }

    echo("✓ Extracted files from bare repo", "green");
    Ok(())
}

/// Post-receive git hook handler.
pub fn cmd_git_hook(paths: &RikuPaths, app: &str) -> Result<()> {
    let app = sanitize_app_name(app);
    let repo_path = paths.git_root.join(&app);
    let app_path = paths.app_root.join(&app);
    let data_path = paths.data_root.join(&app);

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = line?;
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 3 {
            continue;
        }
        let _oldrev = parts[0];
        let newrev = parts[1];
        let _refname = parts[2];

        // Clone repo if app directory doesn't exist or is empty (no Procfile)
        if !app_path.exists() || !app_path.join("Procfile").exists() {
            echo(&format!("-----> Creating app '{}'", app), "green");
            fs::create_dir_all(&app_path)?;
            if !data_path.exists() {
                fs::create_dir_all(&data_path)?;
            }
            let status = Command::new("git")
                .arg("clone")
                .arg("--quiet")
                .arg(&repo_path)
                .arg(&app)
                .current_dir(&paths.app_root)
                .status()?;
            if !status.success() {
                echo("Error: git clone failed.", "red");
            }
        }

        // Call the actual deploy function
        let deltas: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
        crate::deploy::do_deploy(&app, paths, &deltas, Some(newrev))?;
    }

    Ok(())
}

/// Handle git pushes for an app. Sets up bare repo and hook if needed,
/// then delegates to git-receive-pack directly.
pub fn cmd_git_receive_pack(paths: &RikuPaths, app: &str) -> Result<()> {
    let app = sanitize_app_name(app);

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

        // Write post-receive hook that uses PATH to find riku
        // This is more robust than hardcoding the binary path
        let hook_content = r#"#!/usr/bin/env bash
set -e; set -o pipefail;
# Find riku binary from PATH or use common locations
RIKU_BIN="${RIKU_BIN:-$(command -v riku)}"
if [ -z "$RIKU_BIN" ]; then
    # Fallback to common installation paths
    if [ -x "$HOME/.local/bin/riku" ]; then
        RIKU_BIN="$HOME/.local/bin/riku"
    elif [ -x "$HOME/riku" ]; then
        RIKU_BIN="$HOME/riku"
    elif [ -x "/usr/local/bin/riku" ]; then
        RIKU_BIN="/usr/local/bin/riku"
    else
        echo "Error: riku binary not found" >&2
        exit 1
    fi
fi
cat | PIKU_ROOT="${PIKU_ROOT:-$HOME/.riku}" "$RIKU_BIN" git-hook "$2"
"#;
        fs::write(&hook_path, hook_content)?;

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
    let app = sanitize_app_name(app);

    // Call git-upload-pack directly (avoiding shell interpolation)
    let status = Command::new("git-upload-pack")
        .arg(paths.git_root.join(&app))
        .status()?;

    std::process::exit(status.code().unwrap_or(1));
}
