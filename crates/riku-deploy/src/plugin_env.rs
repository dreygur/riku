//! The runtime plugin's `env` output as the last deploy got it, kept in
//! `<env_root>/<app>/PLUGIN_ENV` so `riku restart` gives the workers the
//! same values without running the plugin against a checkout it did not
//! just build.
//!
//! Beside it, `PLUGIN_ENV.source` holds the sha256 of the app's ENV file
//! and server settings file as they were when the plugin's `env` ran. When
//! either changed since, the stored output is stale: the plugin's `env` may
//! read the changed key, so `riku restart` runs it again.
//!
//! Security: both files sit in the app's private env dir and are written
//! `0600`, since a plugin may put secrets in its env. It is read back with
//! the same `KEY=VALUE` parser the plugin's output went through, with no
//! variable expansion.

// Project rule 16 forbids `?`, and this lint asks for exactly that rewrite.
#![allow(clippy::question_mark)]

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use sha2::{Digest, Sha256};

use crate::config::RikuPaths;
use crate::plugins::runtime::parse_env_lines;

const FILE_NAME: &str = "PLUGIN_ENV";
const SOURCE_NAME: &str = "PLUGIN_ENV.source";

/// What [`load`] found.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Stored {
    /// The plugin env, computed from the app's current ENV and settings.
    Current(HashMap<String, String>),
    /// Computed from an ENV or settings file that changed since.
    Stale,
    /// No deploy stored one, or it stored no source hash.
    Missing,
}

fn plugin_env_path(app: &str, paths: &RikuPaths) -> PathBuf {
    paths.env_root.join(app).join(FILE_NAME)
}

fn source_path(app: &str, paths: &RikuPaths) -> PathBuf {
    paths.env_root.join(app).join(SOURCE_NAME)
}

/// The bytes of `path`, empty when it does not exist.
fn file_bytes(path: &Path) -> Result<Vec<u8>> {
    match fs::read(path) {
        Ok(raw) => Ok(raw),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(anyhow!("cannot read {}: {e}", path.display())),
    }
}

/// sha256 of the app's ENV file and server settings file, the inputs a
/// restart can change without a deploy.
fn config_digest(app: &str, paths: &RikuPaths) -> Result<String> {
    let mut hasher = Sha256::new();
    let files = [
        paths.env_root.join(app).join("ENV"),
        paths.app_settings_override(app),
    ];
    for file in &files {
        let raw = match file_bytes(file) {
            Ok(raw) => raw,
            Err(e) => return Err(e),
        };
        hasher.update((raw.len() as u64).to_le_bytes());
        hasher.update(&raw);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn write_file(path: &Path, text: &str) -> Result<()> {
    match crate::util::write_private(path, text.as_bytes()) {
        Ok(()) => Ok(()),
        Err(e) => Err(anyhow!("cannot write {}: {e}", path.display())),
    }
}

/// Store `plugin_env` with the digest of the app's ENV and settings now,
/// replacing what was stored before.
pub(crate) fn save(
    app: &str,
    paths: &RikuPaths,
    plugin_env: &HashMap<String, String>,
) -> Result<()> {
    let env_dir = paths.env_root.join(app);
    if let Err(e) = crate::util::create_private_dir(&env_dir) {
        return Err(e);
    }
    let digest = match config_digest(app, paths) {
        Ok(digest) => digest,
        Err(e) => return Err(e),
    };
    let sorted: BTreeMap<&String, &String> = plugin_env.iter().collect();
    let text: String = sorted
        .into_iter()
        .map(|(key, value)| format!("{key}={value}\n"))
        .collect();
    if let Err(e) = write_file(&plugin_env_path(app, paths), &text) {
        return Err(e);
    }
    write_file(&source_path(app, paths), &format!("{digest}\n"))
}

/// The stored plugin env, if it was computed from the app's current ENV
/// and settings.
pub(crate) fn load(app: &str, paths: &RikuPaths) -> Result<Stored> {
    let stored_digest = match fs::read_to_string(source_path(app, paths)) {
        Ok(text) => text.trim().to_string(),
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Stored::Missing),
        Err(e) => return Err(anyhow!("cannot read the plugin env source: {e}")),
    };
    let path = plugin_env_path(app, paths);
    let raw = match fs::read(&path) {
        Ok(raw) => raw,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Stored::Missing),
        Err(e) => return Err(anyhow!("cannot read {}: {e}", path.display())),
    };
    match config_digest(app, paths) {
        Ok(digest) if digest == stored_digest => {}
        Ok(_) => return Ok(Stored::Stale),
        Err(e) => return Err(e),
    }
    match parse_env_lines(&raw) {
        Ok(env) => Ok(Stored::Current(env)),
        Err(e) => Err(anyhow!("cannot parse {}: {e}", path.display())),
    }
}
