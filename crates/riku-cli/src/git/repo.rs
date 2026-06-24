//! Repository setup helpers: symlinks, hook installation, archive extraction.

use anyhow::{bail, Result};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

use crate::config::RikuPaths;
use crate::util::echo;

/// The post-receive hook script content.
pub(crate) const POST_RECEIVE_HOOK: &str = r#"#!/usr/bin/env bash
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
# Derive app name from the repo directory name (strip .git suffix).
# $2 in post-receive is NOT an argument - all data comes from stdin.
APP="$(basename "$(pwd)" .git)"
REPO_PATH="$(pwd)"
cat | RIKU_ROOT="${RIKU_ROOT:-$HOME/.riku}" "$RIKU_BIN" git-hook "$APP" "$REPO_PATH"
"#;

/// Create symlink from user's bare repo to riku's repos directory.
/// If bare repo exists at ~/app.git, symlink ~/.riku/repos/app.git → ~/app.git
/// Otherwise use ~/.riku/repos/app.git as the canonical location.
pub fn ensure_repo_symlink(paths: &RikuPaths, app: &str) -> Result<()> {
    let home = std::env::var("HOME").unwrap_or_else(|_| String::from("."));
    let user_repo = Path::new(&home).join(format!("{}.git", app));
    let riku_repo = paths.git_root.join(format!("{}.git", app));

    // If user's bare repo exists, create symlink
    if user_repo.exists() {
        // Use symlink_metadata so we detect even dangling symlinks.
        if let Ok(meta) = riku_repo.symlink_metadata() {
            if meta.file_type().is_symlink() {
                // Check if it's already a symlink to the right place
                if let Ok(target) = fs::read_link(&riku_repo) {
                    if target == user_repo {
                        return Ok(()); // Already correctly symlinked
                    }
                }
                // Remove the old symlink
                fs::remove_file(&riku_repo)?;
            } else if meta.file_type().is_dir() {
                // Refuse to remove a real directory — that would destroy data.
                return Err(anyhow::anyhow!(
                    "Repo path '{}' is a real directory, not a symlink; refusing to remove it",
                    riku_repo.display()
                ));
            } else {
                // Regular file at the repo path — remove it.
                fs::remove_file(&riku_repo)?;
            }
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
    fs::write(&hook_path, POST_RECEIVE_HOOK)?;
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
