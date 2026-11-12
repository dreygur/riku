//! Git post-receive hook handler: triggers deployment on push.

use anyhow::{anyhow, Result};
use std::fs;
use std::io::{self, BufRead};
use std::path::Path;
use std::process::Command;

use crate::config::RikuPaths;
use crate::util::display;

use super::hook_repo::{app_repo, check_hook_repo};

/// Whether `app_path` still needs its first clone. Only a missing checkout
/// counts: an app without a Procfile is still a checkout, and cloning into
/// it again fails on the non-empty directory.
fn needs_clone(app_path: &Path) -> bool {
    !app_path.join(".git").exists()
}

fn clone_app(paths: &RikuPaths, app: &str) -> Result<()> {
    let repo_path = app_repo(paths, app);
    let app_path = paths.app_root.join(app);
    let data_path = paths.data_root.join(app);

    for dir in [&app_path, &data_path] {
        if let Err(e) = fs::create_dir_all(dir) {
            return Err(anyhow!("failed to create {}: {e}", dir.display()));
        }
    }
    let status = Command::new("git")
        .arg("clone")
        .arg("--quiet")
        .arg(&repo_path)
        .arg(app)
        .current_dir(&paths.app_root)
        .status();
    // B6: abort this ref's deploy when the clone fails. Continuing would
    // run do_deploy against an empty/half-created app directory.
    match status {
        Ok(status) if status.success() => Ok(()),
        Ok(_) => Err(anyhow!(
            "git clone failed for app '{}' from repo '{}'",
            app,
            repo_path.display()
        )),
        Err(e) => Err(anyhow!("failed to run git clone for app '{app}': {e}")),
    }
}

/// What one pushed ref does.
#[derive(Debug, PartialEq, Eq)]
enum RefPush {
    Deploy,
    /// Stored but not deployed, with the reason shown to the pusher.
    Skip(String),
}

const ZERO_SHA_CHAR: char = '0';
const BRANCH_PREFIX: &str = "refs/heads/";

fn git_in(repo: &Path, args: &[&str]) -> Result<std::process::Output> {
    match Command::new("git")
        .arg("--git-dir")
        .arg(repo)
        .args(args)
        .output()
    {
        Ok(output) => Ok(output),
        Err(e) => Err(anyhow!("failed to run git in {}: {e}", repo.display())),
    }
}

/// The repo's default branch ref: its `HEAD`. When `HEAD` names a branch
/// that does not exist (a repo created with `master` that only ever got
/// `main`), the pushed branch `refname` becomes `HEAD`, so the first branch
/// pushed is the one that deploys.
#[allow(clippy::question_mark)]
fn default_ref(repo: &Path, refname: &str) -> Result<String> {
    let head = match git_in(repo, &["symbolic-ref", "-q", "HEAD"]) {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        }
        Ok(_) => return Err(anyhow!("{} has no symbolic HEAD", repo.display())),
        Err(e) => return Err(e),
    };
    let exists = match git_in(repo, &["rev-parse", "--verify", "-q", &head]) {
        Ok(output) => output.status.success(),
        Err(e) => return Err(e),
    };
    if exists || !refname.starts_with(BRANCH_PREFIX) {
        return Ok(head);
    }
    match git_in(repo, &["symbolic-ref", "HEAD", refname]) {
        Ok(output) if output.status.success() => Ok(refname.to_string()),
        Ok(_) => Err(anyhow!(
            "cannot point HEAD of {} at {refname}",
            repo.display()
        )),
        Err(e) => Err(e),
    }
}

/// Whether a pushed ref deploys: only the default branch does. Tags, other
/// branches and deletions are kept in the repo but not deployed.
fn ref_push(refname: &str, newrev: &str, default: &str) -> RefPush {
    if newrev.chars().all(|c| c == ZERO_SHA_CHAR) {
        return RefPush::Skip(format!("{refname} was deleted"));
    }
    if refname != default {
        let branch = default.strip_prefix(BRANCH_PREFIX).unwrap_or(default);
        return RefPush::Skip(format!(
            "{refname} is not the deploy branch; push to {branch} to deploy"
        ));
    }
    RefPush::Deploy
}

/// Why a hook run failed.
enum HookFailure {
    /// The deploy failed and already printed its failure block.
    Shown(anyhow::Error),
    /// Failed before a deploy could report anything.
    Silent(anyhow::Error),
}

impl From<anyhow::Error> for HookFailure {
    fn from(e: anyhow::Error) -> Self {
        HookFailure::Silent(e)
    }
}

/// Post-receive git hook handler. A failure is also recorded for the push
/// that ran the hook, so `git push` exits non-zero (see `push_status`). A
/// deploy failure the deploy already printed exits 1 without printing it a
/// second time; git ignores a post-receive status anyway.
pub fn cmd_git_hook(paths: &RikuPaths, app: &str, repo_path: Option<&str>) -> Result<()> {
    match deploy_pushed_refs(paths, app, repo_path) {
        Ok(()) => Ok(()),
        Err(HookFailure::Shown(e)) => {
            super::push_status::record_deploy_failure(&e);
            std::process::exit(1);
        }
        Err(HookFailure::Silent(e)) => {
            super::push_status::record_deploy_failure(&e);
            Err(e)
        }
    }
}

#[allow(clippy::question_mark)]
fn deploy_pushed_refs(
    paths: &RikuPaths,
    app: &str,
    repo_path: Option<&str>,
) -> std::result::Result<(), HookFailure> {
    let app = match crate::util::validate_app_name(app) {
        Ok(app) => app,
        Err(e) => return Err(e.into()),
    };

    if let Some(hook_repo) = repo_path {
        if let Err(e) = check_hook_repo(paths, &app, hook_repo) {
            return Err(e.into());
        }
    }

    let app_path = paths.app_root.join(&app);
    let repo = app_repo(paths, &app);

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(line) => line,
            Err(e) => return Err(anyhow!("failed to read pushed refs: {e}").into()),
        };
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 3 {
            continue;
        }
        let newrev = parts[1];
        let refname = parts[2];

        let default = match default_ref(&repo, refname) {
            Ok(default) => default,
            Err(e) => return Err(e.into()),
        };
        if let RefPush::Skip(reason) = ref_push(refname, newrev, &default) {
            display::warn(&format!("Not deploying: {reason}"));
            continue;
        }

        if needs_clone(&app_path) {
            if let Err(e) = clone_app(paths, &app) {
                return Err(e.into());
            }
        }

        let deltas: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
        if let Err(e) = crate::deploy::do_deploy(&app, paths, &deltas, Some(newrev)) {
            return Err(HookFailure::Shown(e));
        }
    }

    Ok(())
}

#[cfg(test)]
#[path = "hook_tests.rs"]
mod tests;
