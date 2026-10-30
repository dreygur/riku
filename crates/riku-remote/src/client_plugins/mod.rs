//! Client-side plugin system for Riku.
//!
//! Client plugins allow extending Riku with local commands that coordinate
//! between the local machine and the Riku server.

pub mod discovery;
pub mod execute;

#[cfg(test)]
mod tests;

pub use discovery::{client_plugin_exists, list_client_plugins};

use anyhow::{anyhow, Result};

use crate::util::display;
use discovery::get_client_plugin_path;
use execute::{execute_plugin, is_executable};

/// Check for and execute a client plugin if it exists.
///
/// Client plugins are looked up in ~/.riku/client-plugins/<command>
///
/// # Arguments
/// * `command` - The command name (e.g., "backup", "open")
/// * `args` - Command line arguments to pass to the plugin
///
/// # Returns
/// * `Ok(true)` - Plugin was found and executed
/// * `Ok(false)` - No plugin found, continue with built-in command
/// * `Err` - Plugin was found but failed to execute
pub fn try_execute_client_plugin(command: &str, args: &[String]) -> Result<bool> {
    let plugin_path = get_client_plugin_path(command)?;

    if !plugin_path.exists() {
        return Ok(false);
    }

    // Check if plugin is executable
    if !is_executable(&plugin_path) {
        return Err(anyhow!(
            "Client plugin '{}' exists but is not executable. Run: chmod +x {}",
            command,
            plugin_path.display()
        ));
    }

    // Execute the plugin
    execute_plugin(&plugin_path, args)?;
    Ok(true)
}

/// Handler for `riku plugin list`.
pub fn cmd_plugin_list() -> Result<()> {
    let plugins = list_client_plugins()?;
    if plugins.is_empty() {
        display::warn("No client plugins installed.");
        display::blank();
        display::note("Install plugins by placing executable scripts in:");
        display::note("  ~/.riku/client-plugins/");
    } else {
        display::section("Available Client Plugins");
        for plugin in plugins {
            display::note(&format!("  {}", plugin));
        }
    }
    Ok(())
}

/// Handler for `riku plugin exists <name>`.
pub fn cmd_plugin_exists(name: &str) -> Result<()> {
    if client_plugin_exists(name)? {
        display::success(&format!("Plugin '{}' is installed and executable.", name));
        std::process::exit(0);
    } else {
        display::warn(&format!("Plugin '{}' not found or not executable.", name));
        std::process::exit(1);
    }
}
