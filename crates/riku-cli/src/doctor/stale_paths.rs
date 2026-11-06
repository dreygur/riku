//! The `riku doctor --fix` step for launch paths that name a riku binary
//! other than [`ROOT_BINARY`]: forced commands in `authorized_keys` and the
//! `post-receive` hook of each repo.
//!
//! Security: only files the running user owns are rewritten, each through an
//! atomic write, and a key line without riku's marker is copied byte for
//! byte. The command text of a riku line is rebuilt from the same
//! [`ROOT_BINARY`] constant the hook generator uses; nothing is taken from
//! the old line except its fingerprint, name and key options.

use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;

use anyhow::{anyhow, Result};

use crate::git::repo::post_receive_hook;
use crate::util::authorized_keys::RIKU_KEY_MARKER;
use crate::util::fs::{write_atomic, write_private};
use crate::util::ROOT_BINARY;

use super::Check;

const KEYS_NAME: &str = "authorized_keys";
const HOOKS_NAME: &str = "post-receive hooks";
const COMMAND_PREFIX: &str = "command=\"";
const HOOK_MODE: u32 = 0o755;

/// Point the program of a riku forced command at [`ROOT_BINARY`]. Returns
/// `None` when the line is not riku's or already right.
#[allow(clippy::question_mark)]
fn fixed_key_line(line: &str) -> Option<String> {
    let Some(rest) = line.strip_prefix(COMMAND_PREFIX) else {
        return None;
    };
    let Some((command, tail)) = rest.split_once('"') else {
        return None;
    };
    if !command.starts_with(RIKU_KEY_MARKER) {
        return None;
    }
    let Some((words, program)) = command.rsplit_once(' ') else {
        return None;
    };
    if program == ROOT_BINARY {
        return None;
    }
    Some(format!("{COMMAND_PREFIX}{words} {ROOT_BINARY}\"{tail}"))
}

fn rewrite_keys(authorized_keys: &Path) -> Result<usize> {
    let text = match fs::read_to_string(authorized_keys) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(anyhow!("cannot read {}: {e}", authorized_keys.display())),
    };
    let mut fixed = 0;
    let rewritten: String = text
        .split_inclusive('\n')
        .map(|raw| {
            let line = raw.strip_suffix('\n').unwrap_or(raw);
            match fixed_key_line(line) {
                Some(new_line) => {
                    fixed += 1;
                    format!("{new_line}{}", &raw[line.len()..])
                }
                None => raw.to_string(),
            }
        })
        .collect();
    if fixed == 0 {
        return Ok(0);
    }
    match write_private(authorized_keys, rewritten.as_bytes()) {
        Ok(()) => Ok(fixed),
        Err(e) => Err(anyhow!("cannot rewrite {}: {e}", authorized_keys.display())),
    }
}

/// Whether `hook` is a riku hook (it calls `git-hook`) that differs from the
/// current generated one, and is owned by this user.
fn is_stale_hook(hook: &Path, current: &str) -> bool {
    let owned = match fs::metadata(hook) {
        Ok(meta) => meta.uid() == nix::unistd::Uid::effective().as_raw(),
        Err(_) => false,
    };
    if !owned {
        return false;
    }
    match fs::read_to_string(hook) {
        Ok(text) => text.contains("git-hook") && text != current,
        Err(_) => false,
    }
}

fn rewrite_hook(hook: &Path, current: &str) -> Result<()> {
    if let Err(e) = write_atomic(hook, current.as_bytes()) {
        return Err(anyhow!("cannot rewrite {}: {e}", hook.display()));
    }
    match fs::set_permissions(hook, fs::Permissions::from_mode(HOOK_MODE)) {
        Ok(()) => Ok(()),
        Err(e) => Err(anyhow!("cannot chmod {}: {e}", hook.display())),
    }
}

#[allow(clippy::question_mark)]
fn rewrite_hooks(git_root: &Path) -> Result<Vec<String>> {
    let entries = match fs::read_dir(git_root) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(anyhow!("cannot read {}: {e}", git_root.display())),
    };
    let current = post_receive_hook();
    let mut fixed = Vec::new();
    for entry in entries {
        let repo = match entry {
            Ok(entry) => entry.path(),
            Err(e) => {
                return Err(anyhow!(
                    "cannot read an entry of {}: {e}",
                    git_root.display()
                ))
            }
        };
        let hook = repo.join("hooks").join("post-receive");
        if !is_stale_hook(&hook, &current) {
            continue;
        }
        if let Err(e) = rewrite_hook(&hook, &current) {
            return Err(e);
        }
        fixed.push(
            repo.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
        );
    }
    fixed.sort();
    Ok(fixed)
}

/// Rewrite stale riku lines in `authorized_keys` and stale hooks under
/// `git_root`, and report what changed. Nothing is reported for a file that
/// needed no change.
pub fn fix_stale_launch_paths(authorized_keys: &Path, git_root: &Path) -> Vec<Check> {
    let mut checks = Vec::new();
    match rewrite_keys(authorized_keys) {
        Ok(0) => {}
        Ok(count) => checks.push(Check::ok(
            KEYS_NAME,
            format!("{count} riku key line(s) now run {ROOT_BINARY}"),
        )),
        Err(e) => checks.push(Check::fail(KEYS_NAME, e.to_string())),
    }
    match rewrite_hooks(git_root) {
        Ok(repos) if repos.is_empty() => {}
        Ok(repos) => checks.push(Check::ok(
            HOOKS_NAME,
            format!("rewritten for {}", repos.join(", ")),
        )),
        Err(e) => checks.push(Check::fail(HOOKS_NAME, e.to_string())),
    }
    checks
}

#[cfg(test)]
#[path = "stale_paths_tests.rs"]
mod tests;
