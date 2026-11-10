//! `rk server add`: point the repo it runs in at the new server.
//!
//! Adds `riku <user@host>:<app>`, where `app` is the repo root's directory
//! name, so the first push needs no git setup by hand. An existing remote
//! named `riku`, or one already pointing at the server, is left alone.
//!
//! Security model: the app name comes from a directory name and goes
//! through `validate_app_name` before it is part of a URL. `git` gets every
//! value as its own argument, never through a shell.

use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{anyhow, Result};

use super::git_remote::read_remotes_in;
use super::types::{Server, SshAddress};
use crate::util::validate_app_name;

const REMOTE_NAME: &str = "riku";

/// What [`add_git_remote`] did.
#[derive(Debug, PartialEq, Eq)]
pub enum RemoteOutcome {
    /// Added the `riku` remote with this URL.
    Added(String),
    /// Left this existing remote as it is.
    Kept(String),
    /// The directory is not inside a git repo.
    NotARepo,
    /// The repo directory name is not a valid app name; the reason.
    InvalidName(String),
}

fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output();
    match output {
        Ok(out) if out.status.success() => {
            Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
        }
        _ => None,
    }
}

fn existing_remote(dir: &Path, address: &SshAddress) -> Option<String> {
    if git(dir, &["remote", "get-url", REMOTE_NAME]).is_some() {
        return Some(REMOTE_NAME.to_string());
    }
    read_remotes_in(dir)
        .into_iter()
        .find(|remote| remote.address == address.as_str())
        .map(|remote| format!("{}:{}", remote.address, remote.app))
}

/// Add the `riku` remote for `server` in the repo containing `repo_dir`.
pub fn add_git_remote(repo_dir: &Path, server: &Server) -> Result<RemoteOutcome> {
    let Some(root) = git(repo_dir, &["rev-parse", "--show-toplevel"]) else {
        return Ok(RemoteOutcome::NotARepo);
    };
    let root = Path::new(&root);
    if let Some(kept) = existing_remote(root, &server.address) {
        return Ok(RemoteOutcome::Kept(kept));
    }
    let dir_name = root
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let app = match validate_app_name(&dir_name) {
        Ok(app) => app,
        Err(e) => return Ok(RemoteOutcome::InvalidName(e.to_string())),
    };
    let url = format!("{}:{app}", server.address.as_str());
    match git(root, &["remote", "add", REMOTE_NAME, &url]) {
        Some(_) => Ok(RemoteOutcome::Added(url)),
        None => Err(anyhow!("git remote add {REMOTE_NAME} {url} failed")),
    }
}

#[cfg(test)]
#[path = "remote_add_tests.rs"]
mod tests;
