//! The last guard before the settings migration writes an app's files: load
//! the app's merged config from the files the migration would write, in a
//! private scratch copy, and compare it with the config the app has now.
//!
//! This catches every way a move can change what the app runs with, not only
//! the cases the per key checks know about: another ENV line expanding
//! `$RIKU_DOMAINS`, an existing override `runtime.name` that would rename a
//! version key, an override `false` meeting an ENV `true`.
//!
//! Security: the scratch copy holds the app's ENV, so it lives under the riku
//! root in a `0700` directory with `0600` files, and is removed afterwards.

#![allow(clippy::question_mark)]

use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::Path;

use anyhow::{anyhow, Result};

use crate::config::app_config;
use crate::config::RikuPaths;
use crate::util::{create_private_dir, write_private};

const SCRATCH_DIR: &str = "scratch";

/// The app's would-be files under `root`, as a riku tree of its own.
fn stage(
    paths: &RikuPaths,
    root: &Path,
    app: &str,
    env_text: &str,
    override_text: &str,
) -> Result<RikuPaths> {
    let staged = RikuPaths::for_tests(root);
    for dir in [staged.app_root.join(app), staged.env_root.join(app)] {
        if let Err(e) = create_private_dir(&dir) {
            return Err(e);
        }
    }
    match fs::read_to_string(paths.app_settings_repo(app)) {
        Ok(repo) => {
            if let Err(e) = write_private(&staged.app_settings_repo(app), repo.as_bytes()) {
                return Err(e);
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(anyhow!("cannot read the repo riku.toml of '{app}': {e}")),
    }
    if let Err(e) = write_private(&staged.env_root.join(app).join("ENV"), env_text.as_bytes()) {
        return Err(e);
    }
    if let Err(e) = write_private(&staged.app_settings_override(app), override_text.as_bytes()) {
        return Err(e);
    }
    Ok(staged)
}

fn differing_keys(
    before: &HashMap<String, String>,
    after: &HashMap<String, String>,
) -> Vec<String> {
    let names: BTreeSet<&String> = before.keys().chain(after.keys()).collect();
    names
        .into_iter()
        .filter(|name| before.get(*name) != after.get(*name))
        .cloned()
        .collect()
}

/// Names of the env keys whose value would differ if the app's ENV became
/// `env_text` and its override `override_text`. Empty means the move is safe.
pub fn changed_keys(
    paths: &RikuPaths,
    app: &str,
    env_text: &str,
    override_text: &str,
) -> Result<Vec<String>> {
    let before = match app_config::load_env(paths, app) {
        Ok(loaded) => loaded.env,
        Err(e) => return Err(e),
    };
    let root = paths
        .riku_root
        .join(SCRATCH_DIR)
        .join(format!("migrate-{app}"));
    let _ = fs::remove_dir_all(&root);
    let after = match stage(paths, &root, app, env_text, override_text) {
        Ok(staged) => app_config::load_env(&staged, app).map(|loaded| loaded.env),
        Err(e) => Err(e),
    };
    let _ = fs::remove_dir_all(&root);
    match after {
        Ok(after) => Ok(differing_keys(&before, &after)),
        Err(e) => Err(e),
    }
}
