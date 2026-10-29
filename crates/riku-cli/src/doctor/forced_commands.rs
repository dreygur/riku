//! Detects and rewrites deploy keys that still use the old forced command.
//!
//! Reading is the default. [`fix`] is the only place doctor writes state, and
//! it delegates to [`riku_util::authorized_keys::migrate_file`], which changes
//! nothing but the old ` $SSH_ORIGINAL_COMMAND` suffix.

use std::io::ErrorKind;
use std::path::Path;

use anyhow::Result;

use crate::util::authorized_keys::{authorized_keys_path, migrate_file, migrate_text};

use super::Check;

/// Warn when any deploy key line would be migrated by `--fix`.
pub fn forced_commands(home: &Path) -> Check {
    check_file(&authorized_keys_path(home))
}

fn check_file(path: &Path) -> Check {
    match std::fs::read_to_string(path) {
        Ok(text) => check_text(&text),
        Err(e) if e.kind() == ErrorKind::NotFound => check_text(""),
        Err(e) => Check::warn(
            "deploy key forced commands",
            format!("could not read {}: {e}", path.display()),
        ),
    }
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
pub fn fix(home: &Path) -> Result<usize> {
    migrate_file(&authorized_keys_path(home))
}

#[cfg(test)]
#[path = "forced_commands_tests.rs"]
mod tests;
