//! `riku init` brings every app repo's post-receive hook up to the current
//! template, so a hook an older riku wrote (a `#!/bin/bash` one, or one
//! with the umask's write bits) does not outlive an upgrade.
//!
//! Security: runs as the deploy user in its own tree, never as root (root
//! never writes in the deploy user's home). Only directories directly in
//! `git_root` that hold a `HEAD` file count as repos, and a symlink in
//! place of a repo is skipped. The hook is replaced by an atomic rename,
//! so a symlink in place of the hook is replaced, never followed.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};

use crate::git::repo::post_receive_hook;
use crate::util::write_atomic;

const HOOK_MODE: u32 = 0o755;

/// What a refresh did.
#[derive(Debug, Default)]
pub(super) struct HookRefresh {
    /// Hooks written because their text or mode differed, or they were missing.
    pub updated: usize,
    /// Repos whose hook could not be written, with why.
    pub failed: Vec<(PathBuf, String)>,
}

fn bare_repos(git_root: &Path) -> Vec<PathBuf> {
    let entries = match fs::read_dir(git_root) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };
    entries
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .map(|entry| entry.path())
        .filter(|path| path.join("HEAD").is_file())
        .collect()
}

fn is_current(hook: &Path, text: &str) -> bool {
    let meta = match fs::symlink_metadata(hook) {
        Ok(meta) => meta,
        Err(_) => return false,
    };
    meta.is_file()
        && meta.permissions().mode() & 0o777 == HOOK_MODE
        && fs::read_to_string(hook).is_ok_and(|current| current == text)
}

#[allow(clippy::question_mark)]
fn write_hook(hook: &Path, text: &str) -> Result<()> {
    if let Some(dir) = hook.parent() {
        if let Err(e) = fs::create_dir_all(dir) {
            return Err(anyhow!("cannot create {}: {e}", dir.display()));
        }
    }
    if let Err(e) = write_atomic(hook, text.as_bytes()) {
        return Err(e);
    }
    match fs::set_permissions(hook, fs::Permissions::from_mode(HOOK_MODE)) {
        Ok(()) => Ok(()),
        Err(e) => Err(anyhow!("cannot chmod {}: {e}", hook.display())),
    }
}

/// Write the current post-receive hook into every repo in `git_root` whose
/// hook differs from it in text or mode. Idempotent.
pub(super) fn refresh_repo_hooks(git_root: &Path) -> HookRefresh {
    let text = post_receive_hook();
    let mut refresh = HookRefresh::default();
    for repo in bare_repos(git_root) {
        let hook = repo.join("hooks").join("post-receive");
        if is_current(&hook, &text) {
            continue;
        }
        match write_hook(&hook, &text) {
            Ok(()) => refresh.updated += 1,
            Err(e) => refresh.failed.push((repo, format!("{e:#}"))),
        }
    }
    refresh
}

#[cfg(test)]
#[path = "repo_hooks_tests.rs"]
mod tests;
