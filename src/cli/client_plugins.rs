//! Client-side plugin system for Riku.
//!
//! Client plugins allow extending Riku with local commands that coordinate
//! between the local machine and the Riku server.

use anyhow::{Result, anyhow};
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::fs;

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

/// Get the path to a client plugin.
fn get_client_plugin_path(command: &str) -> Result<PathBuf> {
    let home = env::var("HOME")
        .map_err(|_| anyhow!("HOME environment variable not set"))?;
    
    let plugin_path = PathBuf::from(&home)
        .join(".riku")
        .join("client-plugins")
        .join(command);
    
    Ok(plugin_path)
}

/// Check if a file is executable.
fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = fs::metadata(path) {
            let mode = metadata.permissions().mode();
            return (mode & 0o100) != 0;
        }
        false
    }
    
    #[cfg(windows)]
    {
        path.extension()
            .map_or(false, |ext| ext == "exe" || ext == "bat" || ext == "cmd")
    }
}

/// Execute a client plugin with the given arguments.
///
/// Plugin interface:
/// - $1: server (e.g., "deploy@server.com")
/// - $2: app name
/// - $3: full command (including subcommands)
/// - $4+: additional arguments
fn execute_plugin(plugin_path: &Path, args: &[String]) -> Result<()> {
    let status = Command::new(plugin_path)
        .args(args)
        .status()?;
    
    if !status.success() {
        return Err(anyhow!(
            "Client plugin '{}' exited with code {}",
            plugin_path.display(),
            status.code().unwrap_or(-1)
        ));
    }
    
    Ok(())
}

/// List available client plugins.
pub fn list_client_plugins() -> Result<Vec<String>> {
    let home = env::var("HOME")
        .map_err(|_| anyhow!("HOME environment variable not set"))?;
    
    let plugins_dir = PathBuf::from(&home)
        .join(".riku")
        .join("client-plugins");
    
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

/// Check if a client plugin exists.
pub fn client_plugin_exists(command: &str) -> Result<bool> {
    let plugin_path = get_client_plugin_path(command)?;
    Ok(plugin_path.exists() && is_executable(&plugin_path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::{Mutex, OnceLock};
    use tempfile::TempDir;
    
    // Mutex to ensure tests don't run in parallel when modifying HOME
    static HOME_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();
    
    fn get_home_mutex() -> &'static Mutex<()> {
        HOME_MUTEX.get_or_init(|| Mutex::new(()))
    }
    
    #[test]
    fn test_get_client_plugin_path() {
        let _guard = get_home_mutex().lock().unwrap();
        let original_home = env::var("HOME").ok();
        
        // Set HOME for testing
        let temp_dir = TempDir::new().unwrap();
        env::set_var("HOME", temp_dir.path());

        let path = get_client_plugin_path("test-plugin").unwrap();
        assert!(path.ends_with(".riku/client-plugins/test-plugin"));
        
        // Restore original HOME
        match original_home {
            Some(home) => env::set_var("HOME", home),
            None => env::remove_var("HOME"),
        }
    }
    
    #[test]
    fn test_is_executable() {
        let temp_dir = TempDir::new().unwrap();
        let script_path = temp_dir.path().join("test-script");
        
        // Create a script
        let mut file = fs::File::create(&script_path).unwrap();
        writeln!(file, "#!/bin/sh").unwrap();
        writeln!(file, "echo test").unwrap();
        
        // Should not be executable yet
        assert!(!is_executable(&script_path));
        
        // Make it executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&script_path).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&script_path, perms).unwrap();
        }
        
        // Should be executable now
        assert!(is_executable(&script_path));
    }
    
    #[test]
    fn test_list_client_plugins_empty() {
        let _guard = get_home_mutex().lock().unwrap();
        let original_home = env::var("HOME").ok();
        
        let temp_dir = TempDir::new().unwrap();
        env::set_var("HOME", temp_dir.path());

        let plugins = list_client_plugins().unwrap();
        assert!(plugins.is_empty());
        
        // Restore original HOME
        match original_home {
            Some(home) => env::set_var("HOME", home),
            None => env::remove_var("HOME"),
        }
    }

    #[test]
    fn test_list_client_plugins() {
        let _guard = get_home_mutex().lock().unwrap();
        let original_home = env::var("HOME").ok();
        
        let temp_dir = TempDir::new().unwrap();
        env::set_var("HOME", temp_dir.path());
        
        // Create plugins directory
        let plugins_dir = temp_dir.path().join(".riku").join("client-plugins");
        fs::create_dir_all(&plugins_dir).unwrap();
        
        // Create a plugin
        let plugin_path = plugins_dir.join("test-plugin");
        let mut file = fs::File::create(&plugin_path).unwrap();
        writeln!(file, "#!/bin/sh").unwrap();
        
        // Make it executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&plugin_path).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&plugin_path, perms).unwrap();
        }
        
        let plugins = list_client_plugins().unwrap();
        assert_eq!(plugins, vec!["test-plugin"]);
        
        // Restore original HOME
        match original_home {
            Some(home) => env::set_var("HOME", home),
            None => env::remove_var("HOME"),
        }
    }
}
