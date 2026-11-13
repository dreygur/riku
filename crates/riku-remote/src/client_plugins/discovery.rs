//! Plugin discovery: locate, list, and check client plugins.
//!
//! Client plugins live in `$HOME/.riku/client-plugins`. The `_in` variants
//! take the home directory, so tests never change `HOME`.

// Project rule 16 forbids `?`, and this lint asks for exactly that rewrite.
#![allow(clippy::question_mark)]

use anyhow::{anyhow, Result};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use super::execute::is_executable;

fn home() -> Result<PathBuf> {
    match env::var("HOME") {
        Ok(home) => Ok(PathBuf::from(home)),
        Err(_) => Err(anyhow!("HOME environment variable not set")),
    }
}

fn plugins_dir_in(home: &Path) -> PathBuf {
    home.join(".riku").join("client-plugins")
}

/// The path of client plugin `command` for the user whose home is `home`.
pub(crate) fn client_plugin_path_in(home: &Path, command: &str) -> PathBuf {
    plugins_dir_in(home).join(command)
}

/// Get the path to a client plugin.
pub(crate) fn get_client_plugin_path(command: &str) -> Result<PathBuf> {
    match home() {
        Ok(home) => Ok(client_plugin_path_in(&home, command)),
        Err(e) => Err(e),
    }
}

/// Check if a client plugin exists and is executable.
pub fn client_plugin_exists(command: &str) -> Result<bool> {
    let plugin_path = match get_client_plugin_path(command) {
        Ok(path) => path,
        Err(e) => return Err(e),
    };
    Ok(plugin_path.exists() && is_executable(&plugin_path))
}

/// List available client plugins.
pub fn list_client_plugins() -> Result<Vec<String>> {
    match home() {
        Ok(home) => list_client_plugins_in(&home),
        Err(e) => Err(e),
    }
}

/// [`list_client_plugins`] for the user whose home is `home`.
pub(crate) fn list_client_plugins_in(home: &Path) -> Result<Vec<String>> {
    let plugins_dir = plugins_dir_in(home);
    if !plugins_dir.exists() {
        return Ok(Vec::new());
    }

    let entries = match fs::read_dir(&plugins_dir) {
        Ok(entries) => entries,
        Err(e) => return Err(anyhow!("cannot read {}: {e}", plugins_dir.display())),
    };
    let mut plugins = Vec::new();
    for entry in entries {
        let path = match entry {
            Ok(entry) => entry.path(),
            Err(e) => return Err(anyhow!("cannot read {}: {e}", plugins_dir.display())),
        };
        if path.is_file() && is_executable(&path) {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                plugins.push(name.to_string());
            }
        }
    }

    plugins.sort();
    Ok(plugins)
}
