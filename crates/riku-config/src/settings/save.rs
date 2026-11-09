//! Changing one key in `settings.toml`.
//!
//! The file is read as a plain TOML table, one key is set or removed, and
//! the result is checked with the same parser `load` uses before it replaces
//! the file, so a save can never leave a file riku would refuse. Every other
//! key is kept; comments are not, because `toml` does not keep them.

use std::fs;
use std::io::ErrorKind;

use anyhow::{anyhow, Result};

use super::parse;
use crate::RikuPaths;

#[allow(clippy::question_mark)]
fn read_table(paths: &RikuPaths) -> Result<toml::Table> {
    let raw = match fs::read_to_string(&paths.settings_file) {
        Ok(raw) => raw,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(toml::Table::new()),
        Err(e) => {
            return Err(anyhow!(
                "failed to read {}: {e}",
                paths.settings_file.display()
            ))
        }
    };
    match raw.parse::<toml::Table>() {
        Ok(table) => Ok(table),
        Err(e) => Err(anyhow!(
            "{}: invalid TOML: {e}",
            paths.settings_file.display()
        )),
    }
}

fn patch(table: &mut toml::Table, section: &str, key: &str, value: Option<&str>) {
    match value {
        Some(value) => {
            let entry = table
                .entry(section.to_string())
                .or_insert_with(|| toml::Value::Table(toml::Table::new()));
            if let toml::Value::Table(inner) = entry {
                inner.insert(key.to_string(), toml::Value::String(value.to_string()));
            }
        }
        None => {
            let now_empty = match table.get_mut(section) {
                Some(toml::Value::Table(inner)) => {
                    inner.remove(key);
                    inner.is_empty()
                }
                _ => false,
            };
            if now_empty {
                table.remove(section);
            }
        }
    }
}

fn replace_file(paths: &RikuPaths, text: &str) -> Result<()> {
    let tmp = paths.settings_file.with_extension("toml.new");
    if let Err(e) = fs::write(&tmp, text) {
        return Err(anyhow!("failed to write {}: {e}", tmp.display()));
    }
    match fs::rename(&tmp, &paths.settings_file) {
        Ok(()) => Ok(()),
        Err(e) => Err(anyhow!(
            "failed to replace {}: {e}",
            paths.settings_file.display()
        )),
    }
}

/// Set `section.key` to `value`, or remove it when `value` is `None`.
#[allow(clippy::question_mark)]
pub fn save_key(paths: &RikuPaths, section: &str, key: &str, value: Option<&str>) -> Result<()> {
    let mut table = match read_table(paths) {
        Ok(table) => table,
        Err(e) => return Err(e),
    };
    patch(&mut table, section, key, value);

    let text = match toml::to_string(&table) {
        Ok(text) => text,
        Err(e) => return Err(anyhow!("failed to encode settings: {e}")),
    };
    if let Err(e) = parse(&text, &paths.settings_file) {
        return Err(e);
    }
    replace_file(paths, &text)
}
