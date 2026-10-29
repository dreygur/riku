//! Rewrites old riku deploy key lines in `authorized_keys`.
//!
//! Older riku versions forced `... riku $SSH_ORIGINAL_COMMAND`, which made
//! sshd's login shell re-split the client's command. Current riku splits
//! `SSH_ORIGINAL_COMMAND` itself, so the suffix is dropped.
//!
//! Security model: this edits a file that gates SSH access. A line is
//! touched only if it starts with `command="`, carries the riku
//! [`RIKU_KEY_MARKER`] inside that quoted value, and the value ends with the
//! old suffix. Only that suffix is removed; every other byte of every line,
//! including line endings and the final newline state, is preserved. The
//! file is replaced by renaming a mode 600 sibling, so a failed write never
//! leaves a truncated `authorized_keys`.

use anyhow::{anyhow, Result};
use std::fs;
use std::io::{ErrorKind, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

/// Marks a forced command written by riku, as opposed to a foreign key.
pub const RIKU_KEY_MARKER: &str = "FINGERPRINT=";

/// Where sshd looks for the authorized keys of the user whose home is `home`.
pub fn authorized_keys_path(home: &Path) -> PathBuf {
    home.join(".ssh").join("authorized_keys")
}

const FORCED_PREFIX: &str = "command=\"";
const OLD_SUFFIX: &str = " $SSH_ORIGINAL_COMMAND";

/// The line without the old suffix, or `None` if it is not an old riku line.
#[allow(clippy::question_mark)]
pub fn migrate_line(line: &str) -> Option<String> {
    let Some(rest) = line.strip_prefix(FORCED_PREFIX) else {
        return None;
    };
    let Some(value_len) = rest.find('"') else {
        return None;
    };
    let value = &rest[..value_len];
    if !value.contains(RIKU_KEY_MARKER) || !value.ends_with(OLD_SUFFIX) {
        return None;
    }
    let keep = FORCED_PREFIX.len() + value_len - OLD_SUFFIX.len();
    Some(format!(
        "{}{}",
        &line[..keep],
        &line[FORCED_PREFIX.len() + value_len..]
    ))
}

/// Migrate every line of `text`, returning the new text and how many changed.
pub fn migrate_text(text: &str) -> (String, usize) {
    let mut out = String::with_capacity(text.len());
    let mut changed = 0;
    for piece in text.split_inclusive('\n') {
        let (line, newline) = match piece.strip_suffix('\n') {
            Some(line) => (line, "\n"),
            None => (piece, ""),
        };
        match migrate_line(line) {
            Some(new) => {
                out.push_str(&new);
                changed += 1;
            }
            None => out.push_str(line),
        }
        out.push_str(newline);
    }
    (out, changed)
}

fn temp_sibling(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".riku-migrate");
    path.with_file_name(name)
}

fn replace_file(path: &Path, contents: &str) -> Result<()> {
    let temp = temp_sibling(path);
    if let Err(e) = fs::remove_file(&temp) {
        if e.kind() != ErrorKind::NotFound {
            return Err(anyhow!("failed to clear {}: {e}", temp.display()));
        }
    }
    let mut file = match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temp)
    {
        Ok(file) => file,
        Err(e) => return Err(anyhow!("failed to create {}: {e}", temp.display())),
    };
    let original = match fs::metadata(path) {
        Ok(meta) => meta,
        Err(e) => return Err(anyhow!("failed to stat {}: {e}", path.display())),
    };
    if let Err(e) = std::os::unix::fs::fchown(&file, Some(original.uid()), Some(original.gid())) {
        let _ = fs::remove_file(&temp);
        return Err(anyhow!("failed to set owner of {}: {e}", temp.display()));
    }
    if let Err(e) = file
        .write_all(contents.as_bytes())
        .and_then(|()| file.sync_all())
    {
        let _ = fs::remove_file(&temp);
        return Err(anyhow!("failed to write {}: {e}", temp.display()));
    }
    if let Err(e) = fs::rename(&temp, path) {
        let _ = fs::remove_file(&temp);
        return Err(anyhow!("failed to replace {}: {e}", path.display()));
    }
    Ok(())
}

/// Migrate the file at `path` in place and return how many lines changed.
/// A missing file counts as zero. The file is rewritten only when the count
/// is nonzero.
pub fn migrate_file(path: &Path) -> Result<usize> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(anyhow!("failed to read {}: {e}", path.display())),
    };
    let (migrated, count) = migrate_text(&text);
    if count == 0 {
        return Ok(0);
    }
    let target = match fs::canonicalize(path) {
        Ok(real) => real,
        Err(e) => return Err(anyhow!("failed to resolve {}: {e}", path.display())),
    };
    match replace_file(&target, &migrated) {
        Ok(()) => Ok(count),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
#[path = "authorized_keys_tests.rs"]
mod tests;
