//! The record of bundled plugin files riku wrote: `<plugin root>/.bundled`,
//! one `<path> <sha256>` line per file, the path relative to the plugin root.
//!
//! `riku init` compares an installed file with its line here to tell a copy
//! riku wrote and nobody changed (safe to refresh on upgrade) from one the
//! user edited.
//!
//! Security: the record is read through [`super::owned_file`], so a symlink
//! or a file owned by someone else is refused, and written like a plugin file
//! (unique temp name created new, renamed over) with mode `0600`. The name is
//! hidden, so runtime discovery skips it.
//!
//! An update holds an exclusive `flock` on `<plugin root>/.bundled.lock`
//! from the read to the rename, so two installs at once (the CLI and the
//! supervisor route) apply one after the other and neither entry is lost.
//! The lock file is opened `0600` with `O_NOFOLLOW`, so a planted symlink
//! fails the update instead of being followed.

// Project rule 16 forbids `?`, and this lint asks for exactly that rewrite.
#![allow(clippy::question_mark)]

use std::collections::BTreeMap;
use std::fs::File;
use std::path::Path;

use anyhow::{anyhow, Result};
use nix::fcntl::{Flock, FlockArg};
use sha2::{Digest, Sha256};

use super::embedded_plugins::EmbeddedPlugin;
use super::owned_file::{read_owned, Owned};
use super::plugin_write::write_plugin_file;
use crate::agent::state::{open_private, Access};

const RECORD_NAME: &str = ".bundled";
const LOCK_NAME: &str = ".bundled.lock";
const RECORD_MODE: u32 = 0o600;

/// Recorded sha256 (lowercase hex) by path relative to the plugin root.
#[derive(Debug, Default)]
pub(crate) struct BundledRecord {
    hashes: BTreeMap<String, String>,
}

impl BundledRecord {
    /// Lines that are not `<path> <hash>` are skipped: they cannot match a
    /// file, so the plugin reads as changed and is left alone.
    fn parse(text: &str) -> Self {
        let hashes = text
            .lines()
            .filter_map(|line| line.split_once(' '))
            .map(|(path, hash)| (path.to_string(), hash.trim().to_string()))
            .collect();
        BundledRecord { hashes }
    }

    fn render(&self) -> String {
        self.hashes
            .iter()
            .map(|(path, hash)| format!("{path} {hash}\n"))
            .collect()
    }

    pub(crate) fn hash_of(&self, path: &str) -> Option<&str> {
        self.hashes.get(path).map(String::as_str)
    }
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// The record in `plugin_root`; empty when there is none yet.
pub(crate) fn load(plugin_root: &Path) -> Result<BundledRecord> {
    match read_owned(plugin_root, RECORD_NAME) {
        Ok(Owned::Missing) => Ok(BundledRecord::default()),
        Ok(Owned::File(bytes)) => Ok(BundledRecord::parse(&String::from_utf8_lossy(&bytes))),
        Ok(Owned::Refused(why)) => Err(anyhow!("refusing the bundled plugin record: {why}")),
        Err(e) => Err(e),
    }
}

/// Blocks until this process holds the record's update lock; dropping the
/// returned guard releases it.
fn lock_record(plugin_root: &Path) -> Result<Flock<File>> {
    let path = plugin_root.join(LOCK_NAME);
    let file = match open_private(&path, Access::Update) {
        Ok(file) => file,
        Err(e) => return Err(anyhow!("cannot open {}: {e}", path.display())),
    };
    match Flock::lock(file, FlockArg::LockExclusive) {
        Ok(locked) => Ok(locked),
        Err((_, errno)) => Err(anyhow!("cannot lock {}: {errno}", path.display())),
    }
}

/// Note the hashes of the files `plugin` just wrote under `plugin_root`.
pub(crate) fn record_written(plugin: &EmbeddedPlugin, plugin_root: &Path) -> Result<()> {
    let _lock = match lock_record(plugin_root) {
        Ok(lock) => lock,
        Err(e) => return Err(e),
    };
    let mut record = match load(plugin_root) {
        Ok(record) => record,
        Err(e) => return Err(e),
    };
    for file in plugin.files {
        record
            .hashes
            .insert(file.relative_path(), sha256_hex(file.bytes));
    }
    write_plugin_file(
        plugin_root,
        RECORD_NAME,
        record.render().as_bytes(),
        RECORD_MODE,
    )
}

#[cfg(test)]
#[path = "bundled_record_tests.rs"]
mod tests;
