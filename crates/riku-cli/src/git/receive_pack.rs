//! Git receive-pack and upload-pack command handlers.
//!
//! Security: the repository path comes from the SSH command line. It is
//! reduced to a validated app name, and the app's one repo
//! `repos/<app>.git` is used whichever spelling was pushed; git runs with an
//! argument vector, no shell.

use anyhow::{anyhow, Result};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::NamedTempFile;

use crate::config::RikuPaths;
use crate::util::display;
use crate::util::validation::{validate_app_name, REPO_SUFFIX};

use super::hook_repo::app_repo;
use super::push_status::{deploy_result, DeployResult, STATUS_ENV};
use super::repo::post_receive_hook;

/// The app's repo a pushed path names. `riku@host:shop` and
/// `riku@host:shop.git` both name `repos/shop.git`, the one repo of app
/// `shop`. App names cannot end in `.git`, so only one suffix is taken off.
/// A trailing `/`, which git accepts in a remote path, goes first.
fn requested_repo(paths: &RikuPaths, requested: &str) -> Result<PathBuf> {
    let requested = requested.trim_end_matches('/');
    let app = requested.strip_suffix(REPO_SUFFIX).unwrap_or(requested);
    match validate_app_name(app) {
        Ok(app) => Ok(app_repo(paths, &app)),
        Err(e) => Err(e),
    }
}

fn install_post_receive_hook(hook_path: &std::path::Path) -> Result<()> {
    if let Some(parent) = hook_path.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            return Err(anyhow!("cannot create {}: {e}", parent.display()));
        }
    }
    if let Err(e) = fs::write(hook_path, post_receive_hook()) {
        return Err(anyhow!("cannot write {}: {e}", hook_path.display()));
    }
    let mode = match fs::metadata(hook_path) {
        Ok(meta) => meta.permissions().mode(),
        Err(e) => return Err(anyhow!("cannot stat {}: {e}", hook_path.display())),
    };
    match fs::set_permissions(hook_path, fs::Permissions::from_mode(mode | 0o755)) {
        Ok(()) => Ok(()),
        Err(e) => Err(anyhow!(
            "cannot make {} executable: {e}",
            hook_path.display()
        )),
    }
}

/// Handle git pushes for an app. Sets up bare repo and hook if needed,
/// then delegates to git-receive-pack directly.
#[allow(clippy::question_mark)]
pub fn cmd_git_receive_pack(paths: &RikuPaths, app: &str) -> Result<()> {
    let riku_repo = match requested_repo(paths, app) {
        Ok(repo) => repo,
        Err(e) => return Err(e),
    };

    // Check for an actual bare-repo marker (HEAD), not just the directory's
    // existence. `git init --bare` always creates the target directory
    // itself, so checking `riku_repo.exists()` after first creating the
    // hooks/ subdirectory would always be true and `git init --bare` would
    // never run, leaving a directory with a hooks/ folder but no git
    // internals (fatal: '<path>' does not appear to be a git repository).
    if !riku_repo.join("HEAD").exists() {
        if let Err(e) = fs::create_dir_all(&paths.git_root) {
            return Err(anyhow!("cannot create {}: {e}", paths.git_root.display()));
        }
        let status = Command::new("git")
            .arg("init")
            .arg("--quiet")
            .arg("--bare")
            .arg(&riku_repo)
            .status();
        match status {
            Ok(status) if status.success() => {}
            Ok(status) => {
                return Err(anyhow!(
                    "git init failed for {} ({status})",
                    riku_repo.display()
                ))
            }
            Err(e) => return Err(anyhow!("cannot run git init: {e}")),
        }
    }

    let hook_path = riku_repo.join("hooks").join("post-receive");
    if !hook_path.exists() {
        if let Err(e) = install_post_receive_hook(&hook_path) {
            return Err(e);
        }
    }

    let status_file = match NamedTempFile::new() {
        Ok(file) => file,
        Err(e) => return Err(anyhow!("cannot create the push status file: {e}")),
    };
    let status = match Command::new("git-receive-pack")
        .arg(&riku_repo)
        .env(STATUS_ENV, status_file.path())
        .status()
    {
        Ok(status) => status,
        Err(e) => return Err(anyhow!("cannot run git-receive-pack: {e}")),
    };
    let (code, message) = push_outcome(status.code(), status_file.path());
    drop(status_file);
    if let Some(message) = message {
        display::error(&message);
    }
    std::process::exit(code);
}

/// The exit code of the push: git-receive-pack's own failure, else 1 when
/// the deploy the hook ran failed. Also what to print: a deploy failure is
/// not printed here, because the hook already showed it to the pusher.
fn push_outcome(receive_pack: Option<i32>, status_file: &Path) -> (i32, Option<String>) {
    match receive_pack {
        Some(0) => {}
        Some(code) => return (code, None),
        None => return (1, None),
    }
    match deploy_result(status_file) {
        DeployResult::Succeeded => (0, None),
        DeployResult::Failed(_) => (1, None),
        DeployResult::Unknown(why) => (1, Some(why)),
    }
}

/// Handle git upload-pack for an app.
#[allow(clippy::question_mark)]
pub fn cmd_git_upload_pack(paths: &RikuPaths, app: &str) -> Result<()> {
    let repo = match requested_repo(paths, app) {
        Ok(repo) => repo,
        Err(e) => return Err(e),
    };

    let status = match Command::new("git-upload-pack").arg(&repo).status() {
        Ok(status) => status,
        Err(e) => return Err(anyhow!("cannot run git-upload-pack: {e}")),
    };
    std::process::exit(status.code().unwrap_or(1));
}

#[cfg(test)]
#[path = "receive_pack_tests.rs"]
mod tests;
