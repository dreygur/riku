//! Git receive-pack and upload-pack command handlers.
//!
//! Security: the repository path comes from the SSH command line. It is
//! reduced to a validated app name plus an optional `.git` before it is
//! joined under `repos/`, and git runs with an argument vector, no shell.

use anyhow::{anyhow, Result};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;
use tempfile::NamedTempFile;

use crate::config::RikuPaths;
use crate::util::echo;
use crate::util::validation::{validate_app_name, REPO_SUFFIX};

use super::push_status::{deploy_failure, STATUS_ENV};
use super::repo::{ensure_repo_symlink, post_receive_hook};

/// The directory under `repos/` a pushed path names. `riku@host:shop` and
/// `riku@host:shop.git` both deploy app `shop`, and their repositories stay
/// at `repos/shop` and `repos/shop.git` as before. App names cannot end in
/// `.git`, so only one suffix is taken off. A trailing `/`, which git
/// accepts in a remote path, goes first.
fn repo_dir_name(requested: &str) -> Result<String> {
    let requested = requested.trim_end_matches('/');
    let (app, suffix) = match requested.strip_suffix(REPO_SUFFIX) {
        Some(app) => (app, REPO_SUFFIX),
        None => (requested, ""),
    };
    match validate_app_name(app) {
        Ok(app) => Ok(format!("{app}{suffix}")),
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
    let repo = match repo_dir_name(app) {
        Ok(repo) => repo,
        Err(e) => return Err(e),
    };

    if let Err(e) = ensure_repo_symlink(paths, &repo) {
        return Err(e);
    }

    let riku_repo = paths.git_root.join(&repo);

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
            .arg(&repo)
            .current_dir(&paths.git_root)
            .status();
        match status {
            Ok(status) if status.success() => {}
            Ok(_) => echo("git init failed.", "red"),
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
    let code = push_exit_code(status.code(), status_file.path());
    drop(status_file);
    std::process::exit(code);
}

/// git-receive-pack's own failure, else 1 when the deploy the hook ran
/// failed. Its stderr reaches the pushing client, so the reason goes there.
fn push_exit_code(receive_pack: Option<i32>, status_file: &Path) -> i32 {
    match receive_pack {
        Some(0) => {}
        Some(code) => return code,
        None => return 1,
    }
    match deploy_failure(status_file) {
        Some(reason) => {
            echo(
                &format!("The push was received, but the deploy failed: {reason}"),
                "red",
            );
            1
        }
        None => 0,
    }
}

/// Handle git upload-pack for an app.
#[allow(clippy::question_mark)]
pub fn cmd_git_upload_pack(paths: &RikuPaths, app: &str) -> Result<()> {
    let repo = match repo_dir_name(app) {
        Ok(repo) => repo,
        Err(e) => return Err(e),
    };

    let status = match Command::new("git-upload-pack")
        .arg(paths.git_root.join(&repo))
        .status()
    {
        Ok(status) => status,
        Err(e) => return Err(anyhow!("cannot run git-upload-pack: {e}")),
    };
    std::process::exit(status.code().unwrap_or(1));
}

#[cfg(test)]
#[path = "receive_pack_tests.rs"]
mod tests;
