//! Plugin discovery: locate, list, and check client plugins.

use anyhow::{anyhow, Result};
use std::env;
use std::fs;
use std::path::PathBuf;

use super::execute::is_executable;

/// Get the path to a client plugin.
pub(crate) fn get_client_plugin_path(command: &str) -> Result<PathBuf> {
    let home = env::var("HOME").map_err(|_| anyhow!("HOME environment variable not set"))?;

    let plugin_path = PathBuf::from(&home)
        .join(".riku")
        .join("client-plugins")
        .join(command);

    Ok(plugin_path)
}

/// Check if a client plugin exists and is executable.
pub fn client_plugin_exists(command: &str) -> Result<bool> {
    let plugin_path = get_client_plugin_path(command)?;
    Ok(plugin_path.exists() && is_executable(&plugin_path))
}

/// List available client plugins.
pub fn list_client_plugins() -> Result<Vec<String>> {
    let home = env::var("HOME").map_err(|_| anyhow!("HOME environment variable not set"))?;

    let plugins_dir = PathBuf::from(&home).join(".riku").join("client-plugins");

    if !plugins_dir.exists() {
        return Ok(Vec::new());
    }

    let mut plugins = Vec::new();

    for entry in fs::read_dir(&plugins_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() && is_executable(&path) {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                plugins.push(name.to_string());
            }
        }
    }

    plugins.sort();
    Ok(plugins)
}
