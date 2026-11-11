//! `rk server add`, after the entry is saved: authorize this machine's key
//! on the server and point the current repo at it.
//!
//! The entry is saved first and kept even when the key step fails, so a
//! rerun of the printed server command is all that is left to do.

use std::path::Path;

use anyhow::{anyhow, Result};

use super::key_install::{install_key, key_path, read_public_key};
use super::remote_add::{add_git_remote, RemoteOutcome};
use super::types::Server;
use crate::util::display;
use crate::util::public_key::PublicKey;

const NO_KEY: &str = "No public key found in ~/.ssh; pass --key PATH to authorize one for pushes";

/// The key to authorize: `--key`, else a default key in `~/.ssh`. A key file
/// that cannot be read is an error, raised before anything is saved; no
/// default key is `None`.
pub fn chosen_key(explicit: Option<&Path>) -> Result<Option<PublicKey>> {
    let Some(home) = dirs::home_dir() else {
        return Err(anyhow!("could not determine the home directory"));
    };
    match key_path(explicit, &home) {
        Some(path) => read_public_key(&path).map(Some),
        None => Ok(None),
    }
}

fn report_remote(outcome: Result<RemoteOutcome>) {
    match outcome {
        Ok(RemoteOutcome::Added(url)) => display::step_done("Git remote", &format!("riku {url}")),
        Ok(RemoteOutcome::Kept(remote)) => {
            display::step_done("Git remote", &format!("kept {remote}"))
        }
        Ok(RemoteOutcome::NotARepo) => {}
        Ok(RemoteOutcome::InvalidName(reason)) => display::warn(&format!(
            "No git remote added: the repo directory is not an app name ({reason})"
        )),
        Err(e) => display::warn(&format!("No git remote added: {e}")),
    }
}

/// Authorize `key` on the saved `server` and add the git remote in the repo
/// at `repo_dir`. The remote step runs even when the key step fails; the
/// key step's error is returned after it.
pub fn finish_add(repo_dir: &Path, server: &Server, key: Option<&PublicKey>) -> Result<()> {
    display::step_done("Server", &format!("{} saved", server.name.as_str()));
    let installed = match key {
        Some(key) => install_key(server, key),
        None => {
            display::warn(NO_KEY);
            Ok(())
        }
    };
    report_remote(add_git_remote(repo_dir, server));
    if installed.is_ok() {
        display::outro(&format!("{} added", server.name.as_str()));
        display::summary(&[("Deploy", "git push riku main")]);
    }
    installed
}
