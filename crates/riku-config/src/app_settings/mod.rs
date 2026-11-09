//! `riku.toml`: per app riku settings.
//!
//! The file lives in the app repo (committed, read from the checkout) or on
//! the server as an override at `<env_root>/<app>/riku.toml`. Each key maps to
//! a `RIKU_*` env key through [`keys::KEYS`]; `[processes]` and `[cron]`
//! replace the Procfile.
//!
//! Security model: the repo file is pushed input, the override is operator
//! input. Repo values are checked by charset in `values.rs`; server only keys
//! (limits, isolation, binding, secrets) are refused in the repo file;
//! process sections are refused in the override. Errors name the file, line
//! and key, never a value.

#![allow(clippy::question_mark)]

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use anyhow::{anyhow, Result};

mod edit;
pub mod keys;
mod location;
mod parse;
mod processes;
mod values;

use keys::{EnvName, Key};
use values::Value;

/// File name of both the repo file and the server override.
pub const FILE_NAME: &str = "riku.toml";

/// Which file is being read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// `riku.toml` in the app repo.
    Repo,
    /// The server override, edited with `riku settings`.
    Server,
}

/// Why a file is refused, before it is tied to a file and line.
pub(crate) struct Problem {
    section: String,
    key: Option<String>,
    message: String,
}

impl Problem {
    fn at(section: &str, key: &str, message: String) -> Problem {
        Problem {
            section: section.to_string(),
            key: Some(key.to_string()),
            message,
        }
    }
}

/// One parsed `riku.toml`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AppSettings {
    values: Vec<(&'static Key, Value)>,
    processes: Option<BTreeMap<String, String>>,
}

impl AppSettings {
    fn value_of(&self, path: &str) -> Option<&Value> {
        self.values
            .iter()
            .find(|(key, _)| key.path() == path)
            .map(|(_, value)| value)
    }

    fn runtime_name(&self) -> Option<String> {
        match self.value_of("runtime.name") {
            Some(Value::Text(name)) => Some(name.clone()),
            _ => None,
        }
    }

    fn put(&mut self, key: &'static Key, value: Value) {
        self.values
            .retain(|(existing, _)| !std::ptr::eq(*existing, key));
        self.values.push((key, value));
    }

    /// Parse `raw`. `file` names the file in error messages.
    pub fn parse(raw: &str, file: &Path, source: Source) -> Result<AppSettings> {
        let root: toml::Table = match raw.parse() {
            Ok(root) => root,
            Err(e) => return Err(location::syntax(raw, file, &e)),
        };
        match AppSettings::from_table(&root, source) {
            Ok(settings) => Ok(settings),
            Err(problem) => Err(location::located(raw, file, problem)),
        }
    }

    /// Read and parse `file`; `None` when it does not exist.
    pub fn load(file: &Path, source: Source) -> Result<Option<AppSettings>> {
        match fs::read_to_string(file) {
            Ok(raw) => match AppSettings::parse(&raw, file, source) {
                Ok(settings) => Ok(Some(settings)),
                Err(e) => Err(e),
            },
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
            Err(e) => Err(anyhow!("failed to read {}: {e}", file.display())),
        }
    }

    /// The `RIKU_*` env keys this file sets.
    pub fn env(&self) -> HashMap<String, String> {
        let mut env = HashMap::new();
        for (key, value) in &self.values {
            let name = match (key.env, self.runtime_name()) {
                (EnvName::Fixed(name), _) => name.to_string(),
                (EnvName::RuntimeVersion, Some(runtime)) => keys::runtime_version_env(&runtime),
                (EnvName::RuntimeVersion, None) => continue,
            };
            if let Some(rendered) = values::render(key, value) {
                env.insert(name, rendered);
            }
        }
        env
    }

    /// Env keys this file turns off: flags set to `false`, which render as
    /// no key. A later layer removes them, so a `false` in the server
    /// override beats a `true` from the repo file or ENV.
    pub fn cleared_env(&self) -> Vec<String> {
        self.values
            .iter()
            .filter(|(key, value)| key.kind == keys::Kind::Flag && *value == Value::Bool(false))
            .filter_map(|(key, _)| match key.env {
                EnvName::Fixed(name) => Some(name.to_string()),
                EnvName::RuntimeVersion => None,
            })
            .collect()
    }

    /// `[processes]` plus `[cron]` in the shape `parse_procfile` returns;
    /// `None` when neither section is present.
    pub fn processes(&self) -> Option<HashMap<String, String>> {
        self.processes.as_ref().map(|processes| {
            processes
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        })
    }
}

#[cfg(test)]
#[path = "app_settings_tests.rs"]
mod tests;
