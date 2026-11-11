//! Repository setup helpers: symlinks, hook installation, archive extraction.

use anyhow::{bail, Result};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

use crate::config::RikuPaths;
use crate::util::{echo, ROOT_BINARY};

/// The post-receive hook every repo gets. It runs [`ROOT_BINARY`] and never
/// searches `PATH` or the home directory. The app is the repo directory
/// name without `.git`.
///
/// `riku git-hook` reads the "oldrev newrev refname" lines directly from its
/// own stdin, so the hook must not `read` them first: that would starve the
/// child of input and silently deploy nothing on a single-ref push.
pub(crate) fn post_receive_hook() -> String {
    format!(
        r#"#!/bin/bash
APP=$(basename "$(pwd)" .git)
REPO_PATH="$(pwd)"
BINARY="{ROOT_BINARY}"
if [ -x "$BINARY" ]; then
    exec "$BINARY" git-hook "$APP" "$REPO_PATH"
else
    echo " !     Riku binary not found at $BINARY" >&2
    exit 1
fi
"#
    )
}

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
                // Refuse to remove a real directory: that would destroy data.
                return Err(anyhow::anyhow!(
                    "Repo path '{}' is a real directory, not a symlink; refusing to remove it",
                    riku_repo.display()
                ));
            } else {
                // Regular file at the repo path: remove it.
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
    fs::write(&hook_path, post_receive_hook())?;
    fs::set_permissions(&hook_path, PermissionsExt::from_mode(0o755))?;

    echo(
        &format!("Created post-receive hook in {}", repo_path.display()),
        "green",
    );
    Ok(())
}

/// Extract files from bare repo to app directory using git archive.
///
/// Runs `git archive` and `tar` directly with argument vectors (no shell), so
/// neither `app` nor `bare_repo` can inject shell metacharacters into the
/// extraction command: `app` only ever reaches a path join, never a string
/// interpolated into a command line.
pub fn extract_bare_repo_to_app(bare_repo: &Path, app: &str, paths: &RikuPaths) -> Result<()> {
    let app_dir = paths.app_root.join(app);

    // Create app directory
    if app_dir.exists() {
        if let Err(e) = fs::remove_dir_all(&app_dir) {
            return Err(anyhow::anyhow!(
                "failed to remove existing app directory {}: {e}",
                app_dir.display()
            ));
        }
    }
    if let Err(e) = fs::create_dir_all(&app_dir) {
        return Err(anyhow::anyhow!(
            "failed to create app directory {}: {e}",
            app_dir.display()
        ));
    }

    let mut git_archive = match Command::new("git")
        .args(["archive", "--format=tar", "HEAD"])
        .current_dir(bare_repo)
        .stdout(std::process::Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(e) => return Err(anyhow::anyhow!("failed to spawn git archive: {e}")),
    };

    let archive_stdout = match git_archive.stdout.take() {
        Some(stdout) => stdout,
        None => return Err(anyhow::anyhow!("failed to capture git archive stdout")),
    };

    let tar_status = Command::new("tar")
        .arg("-xf")
        .arg("-")
        .arg("-C")
        .arg(&app_dir)
        .stdin(archive_stdout)
        .status();

    let git_status = git_archive.wait();

    match (git_status, tar_status) {
        (Ok(git), Ok(tar)) if git.success() && tar.success() => {}
        (Ok(git), Ok(tar)) => {
            bail!(
                "Failed to extract files from bare repo (git archive: {}, tar: {})",
                git,
                tar
            );
        }
        (Err(e), _) => return Err(anyhow::anyhow!("git archive failed to run: {e}")),
        (_, Err(e)) => return Err(anyhow::anyhow!("tar failed to run: {e}")),
    }

    echo("Extracted files from bare repo", "green");
    Ok(())
}

#[cfg(test)]
#[path = "repo_tests.rs"]
mod tests;
