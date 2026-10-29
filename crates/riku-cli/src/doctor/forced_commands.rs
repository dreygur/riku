//! Detects and rewrites deploy keys that still use the old forced command.
//!
//! Reading is the default. [`fix`] is the only place doctor writes state, and
//! it delegates to [`riku_util::authorized_keys::migrate_file`], which changes
//! nothing but the old ` $SSH_ORIGINAL_COMMAND` suffix.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};

use crate::util::authorized_keys::{migrate_file, migrate_text};

use super::Check;

fn authorized_keys_path() -> Result<PathBuf> {
    match std::env::var("HOME") {
        Ok(home) => Ok(Path::new(&home).join(".ssh/authorized_keys")),
        Err(_) => Err(anyhow!("HOME not set, cannot locate authorized_keys")),
    }
}

/// Warn when any deploy key line would be migrated by `--fix`.
pub fn forced_commands() -> Check {
    let path = match authorized_keys_path() {
        Ok(path) => path,
        Err(e) => return Check::warn("deploy key forced commands", e.to_string()),
    };
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    check_text(&text)
}

fn check_text(text: &str) -> Check {
    let (_, old) = migrate_text(text);
    if old == 0 {
        return Check::ok("deploy key forced commands", "up to date");
    }
    Check::warn(
        "deploy key forced commands",
        format!("{old} deploy keys use the old forced command, run: riku doctor --fix"),
    )
}

/// Migrate this user's `authorized_keys` and return the number of lines changed.
#[allow(clippy::question_mark)]
pub fn fix() -> Result<usize> {
    let path = match authorized_keys_path() {
        Ok(path) => path,
        Err(e) => return Err(e),
    };
    migrate_file(&path)
}

#[cfg(test)]
#[path = "forced_commands_tests.rs"]
mod tests;
