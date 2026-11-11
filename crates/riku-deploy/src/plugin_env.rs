//! The runtime plugin's `env` output as the last deploy got it, kept in
//! `<env_root>/<app>/PLUGIN_ENV` so `riku restart` gives the workers the
//! same values without running the plugin against a checkout it did not
//! just build.
//!
//! Security: the file sits in the app's private env dir and is written
//! `0600`, since a plugin may put secrets in its env. It is read back with
//! the same `KEY=VALUE` parser the plugin's output went through, with no
//! variable expansion.

// Project rule 16 forbids `?`, and this lint asks for exactly that rewrite.
#![allow(clippy::question_mark)]

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;

use anyhow::{anyhow, Result};

use crate::config::RikuPaths;
use crate::plugins::runtime::parse_env_lines;

const FILE_NAME: &str = "PLUGIN_ENV";

fn plugin_env_path(app: &str, paths: &RikuPaths) -> PathBuf {
    paths.env_root.join(app).join(FILE_NAME)
}

/// Store `plugin_env`, replacing what an earlier deploy stored.
pub(crate) fn save(
    app: &str,
    paths: &RikuPaths,
    plugin_env: &HashMap<String, String>,
) -> Result<()> {
    let env_dir = paths.env_root.join(app);
    if let Err(e) = crate::util::create_private_dir(&env_dir) {
        return Err(e);
    }
    let sorted: BTreeMap<&String, &String> = plugin_env.iter().collect();
    let text: String = sorted
        .into_iter()
        .map(|(key, value)| format!("{key}={value}\n"))
        .collect();
    let path = plugin_env_path(app, paths);
    match crate::util::write_private(&path, text.as_bytes()) {
        Ok(()) => Ok(()),
        Err(e) => Err(anyhow!("cannot write {}: {e}", path.display())),
    }
}

/// The stored plugin env, or `None` when no deploy stored one (an app last
/// deployed by a riku that did not keep it).
pub(crate) fn load(app: &str, paths: &RikuPaths) -> Result<Option<HashMap<String, String>>> {
    let path = plugin_env_path(app, paths);
    let raw = match fs::read(&path) {
        Ok(raw) => raw,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(anyhow!("cannot read {}: {e}", path.display())),
    };
    match parse_env_lines(&raw) {
        Ok(env) => Ok(Some(env)),
        Err(e) => Err(anyhow!("cannot parse {}: {e}", path.display())),
    }
}
