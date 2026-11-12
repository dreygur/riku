//! Bringing installed bundled plugins up to date on `riku init`.
//!
//! A copy riku wrote and nobody changed since (its hash matches the bundled
//! record) is rewritten with this riku's version. A copy that differs from
//! the record, or has no record at all, counts as changed by the user: it
//! is left alone and reported, so an upgrade never discards a user's edit.
//!
//! Security: files are read through [`super::owned_file`], so only regular
//! files owned by the effective user, under directories it owns, are ever
//! considered; a symlink anywhere on the way is refused, not followed. A
//! refresh goes through [`write_out`], which renames a new file over the
//! old entry and so never writes through a link. Init calls this only as the
//! deploy user, in that user's own plugin directory; root never does.

// Project rule 16 forbids `?`, and this lint asks for exactly that rewrite.
#![allow(clippy::question_mark)]

use std::path::Path;

use anyhow::Result;

use super::bundled_record::{self, sha256_hex, BundledRecord};
use super::embedded_plugins::{write_out, EmbeddedFile, EmbeddedPlugin, EMBEDDED};
use super::owned_file::{read_owned, Owned};

/// One installed file compared with the record and this riku's copy.
enum FileState {
    /// Not on disk; `recorded` when riku wrote it once, so a user removed it.
    Missing {
        recorded: bool,
    },
    Refused(String),
    /// Differs from the record, or has none.
    Modified,
    Outdated,
    Current,
}

/// What to do with a whole plugin.
enum PluginState {
    NotInstalled,
    Refused(String),
    Modified,
    Outdated,
    Current,
}

fn file_state(root: &Path, record: &BundledRecord, file: &EmbeddedFile) -> Result<FileState> {
    let path = file.relative_path();
    let recorded = record.hash_of(&path);
    let bytes = match read_owned(root, &path) {
        Ok(Owned::File(bytes)) => bytes,
        Ok(Owned::Missing) => {
            return Ok(FileState::Missing {
                recorded: recorded.is_some(),
            })
        }
        Ok(Owned::Refused(why)) => return Ok(FileState::Refused(why)),
        Err(e) => return Err(e),
    };
    let on_disk = sha256_hex(&bytes);
    Ok(match recorded {
        None => FileState::Modified,
        Some(hash) if hash != on_disk => FileState::Modified,
        Some(_) if on_disk != sha256_hex(file.bytes) => FileState::Outdated,
        Some(_) => FileState::Current,
    })
}

/// The plugin's state from its files'. A refusal wins over an edit, so a
/// plugin is rewritten only when every file is riku's own, unchanged copy
/// (or one this riku adds).
fn plugin_state(states: &[FileState]) -> PluginState {
    if states
        .iter()
        .all(|state| matches!(state, FileState::Missing { .. }))
    {
        return PluginState::NotInstalled;
    }
    for state in states {
        if let FileState::Refused(why) = state {
            return PluginState::Refused(why.clone());
        }
    }
    let any = |wanted: fn(&FileState) -> bool| states.iter().any(wanted);
    if any(|state| {
        matches!(
            state,
            FileState::Modified | FileState::Missing { recorded: true }
        )
    }) {
        PluginState::Modified
    } else if any(|state| {
        matches!(
            state,
            FileState::Outdated | FileState::Missing { recorded: false }
        )
    }) {
        PluginState::Outdated
    } else {
        PluginState::Current
    }
}

fn inspect_plugin(
    root: &Path,
    record: &BundledRecord,
    plugin: &EmbeddedPlugin,
) -> Result<PluginState> {
    let mut states = Vec::with_capacity(plugin.files.len());
    for file in plugin.files {
        match file_state(root, record, file) {
            Ok(state) => states.push(state),
            Err(e) => return Err(e),
        }
    }
    Ok(plugin_state(&states))
}

/// What a refresh did, by plugin name.
#[derive(Debug, Default, PartialEq)]
pub struct RefreshReport {
    /// Rewritten with this riku's version.
    pub refreshed: Vec<&'static str>,
    /// Changed since riku wrote them, or with no record of riku writing
    /// them; left alone.
    pub modified: Vec<&'static str>,
    /// Not a regular file the user owns (a symlink, say), with why; left alone.
    pub refused: Vec<(&'static str, String)>,
}

/// Refresh every bundled plugin installed in `plugin_root` that riku wrote
/// and nobody changed, and report the rest.
pub fn refresh_bundled(plugin_root: &Path) -> Result<RefreshReport> {
    let record = match bundled_record::load(plugin_root) {
        Ok(record) => record,
        Err(e) => return Err(e),
    };
    let mut report = RefreshReport::default();
    for plugin in EMBEDDED {
        let state = match inspect_plugin(plugin_root, &record, plugin) {
            Ok(state) => state,
            Err(e) => return Err(e),
        };
        match state {
            PluginState::NotInstalled | PluginState::Current => {}
            PluginState::Refused(why) => report.refused.push((plugin.name, why)),
            PluginState::Modified => report.modified.push(plugin.name),
            PluginState::Outdated => {
                if let Err(e) = write_out(plugin, plugin_root) {
                    return Err(e);
                }
                report.refreshed.push(plugin.name);
            }
        }
    }
    Ok(report)
}

#[cfg(test)]
#[path = "plugin_refresh_tests.rs"]
mod tests;
