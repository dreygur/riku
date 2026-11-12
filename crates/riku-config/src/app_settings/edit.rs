//! Changing a server override: `set`, `unset`, `get`, `entries`, `to_toml`.

use anyhow::{anyhow, Result};

use super::keys::{self, KEYS};
use super::values::{self, Value};
use super::{AppSettings, Source};

const CRON_KIND_PREFIX: &str = "cron-";

fn display(value: &Value) -> String {
    match value {
        Value::Text(text) => text.clone(),
        Value::Int(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::List(items) => items.join(","),
    }
}

fn section_table<'a>(root: &'a mut toml::Table, section: &str) -> &'a mut toml::Table {
    let entry = root
        .entry(section.to_string())
        .or_insert_with(|| toml::Value::Table(toml::Table::new()));
    if !entry.is_table() {
        *entry = toml::Value::Table(toml::Table::new());
    }
    match entry {
        toml::Value::Table(table) => table,
        _ => unreachable!("entry was just made a table"),
    }
}

impl AppSettings {
    /// Set `path` (`section.name`) from command line text. List values are
    /// comma separated. Server rules apply: this edits the override.
    pub fn set(&mut self, path: &str, text: &str) -> Result<()> {
        let key = match keys::find(path) {
            Some(key) => key,
            None => return Err(anyhow!("unknown key {path}")),
        };
        let raw = match values::parse_text(key, text) {
            Some(raw) => raw,
            None => return Err(anyhow!("{path}: value has the wrong type")),
        };
        let value = match values::check(key, &raw, Source::Server) {
            Ok(value) => value,
            Err(message) => return Err(anyhow!("{message}")),
        };
        self.put(key, value);
        match self.check_runtime_version() {
            Ok(()) => Ok(()),
            Err(problem) => {
                self.unset(path);
                Err(anyhow!("{}", problem.message))
            }
        }
    }

    /// Remove `path`; nothing happens when it is not set.
    pub fn unset(&mut self, path: &str) {
        self.values.retain(|(key, _)| key.path() != path);
    }

    /// The value at `path` as text (lists joined with `,`).
    pub fn get(&self, path: &str) -> Option<String> {
        self.value_of(path).map(display)
    }

    /// Every set key in table order as `(path, text, secret)`.
    pub fn entries(&self) -> Vec<(String, String, bool)> {
        KEYS.iter()
            .filter_map(|key| {
                self.value_of(&key.path())
                    .map(|value| (key.path(), display(value), key.secret))
            })
            .collect()
    }

    /// The file text for these settings.
    pub fn to_toml(&self) -> Result<String> {
        let mut root = toml::Table::new();
        for (key, value) in &self.values {
            section_table(&mut root, key.section)
                .insert(key.name.to_string(), values::to_toml(value));
        }
        if let Some(processes) = &self.processes {
            for (kind, command) in processes {
                let (section, name) = match kind.strip_prefix(CRON_KIND_PREFIX) {
                    Some(name) => ("cron", name),
                    None => ("processes", kind.as_str()),
                };
                section_table(&mut root, section)
                    .insert(name.to_string(), toml::Value::String(command.clone()));
            }
        }
        match toml::to_string(&root) {
            Ok(text) => Ok(text),
            Err(e) => Err(anyhow!("failed to encode riku.toml: {e}")),
        }
    }
}
