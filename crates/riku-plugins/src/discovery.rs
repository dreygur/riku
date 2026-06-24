//! Plugin discovery and execution primitives.
//!
//! Handles locating executable plugins in `~/.riku/plugins/` and
//! running them as child processes.

use anyhow::Result;
use std::fs;

use crate::config::RikuPaths;

/// Validate plugin name does not contain path separators or traversal sequences.
pub fn validate_plugin_name(plugin_name: &str) -> Result<()> {
    if plugin_name.contains('/')
        || plugin_name.contains('\\')
        || plugin_name.contains("..")
        || plugin_name.is_empty()
    {
        return Err(anyhow::anyhow!(
            "Invalid plugin name '{}': must not contain path separators or '..'",
            plugin_name
        ));
    }
    Ok(())
}

/// Scan the plugins directory and return a list of available plugins.
pub fn list_plugins(paths: &RikuPaths) -> Result<Vec<String>> {
    let mut plugins = Vec::new();

    if !paths.plugin_root.exists() {
        return Ok(plugins);
    }

    for entry in fs::read_dir(&paths.plugin_root)? {
        let entry = entry?;
        let file_type = entry.file_type()?;

        if file_type.is_file() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let metadata = entry.metadata()?;
                let permissions = metadata.permissions();

                // Only list executable files
                if permissions.mode() & 0o111 != 0 {
                    if let Some(name) = entry.file_name().to_str() {
                        plugins.push(name.to_string());
                    }
                }
            }

            #[cfg(not(unix))]
            {
                if let Some(name) = entry.file_name().to_str() {
                    plugins.push(name.to_string());
                }
            }
        }
    }

    Ok(plugins)
}

/// Check if a plugin exists and is executable.
pub fn plugin_exists(plugin_name: &str, paths: &RikuPaths) -> bool {
    if validate_plugin_name(plugin_name).is_err() {
        return false;
    }
    let plugin_path = paths.plugin_root.join(plugin_name);

    if !plugin_path.exists() {
        return false;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = fs::metadata(&plugin_path) {
            let permissions = metadata.permissions();
            permissions.mode() & 0o111 != 0
        } else {
            false
        }
    }

    #[cfg(not(unix))]
    {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_list_plugins_empty_directory() {
        let temp_dir = TempDir::new().unwrap();
        let paths =
            crate::config::RikuPaths::from_dirs(temp_dir.path().join(".riku"), temp_dir.path());
        fs::create_dir_all(&paths.plugin_root).unwrap();
        let plugins = list_plugins(&paths).unwrap();
        assert!(plugins.is_empty());
    }

    #[test]
    fn test_plugin_exists() {
        let temp_dir = TempDir::new().unwrap();
        let paths =
            crate::config::RikuPaths::from_dirs(temp_dir.path().join(".riku"), temp_dir.path());
        fs::create_dir_all(&paths.plugin_root).unwrap();

        let plugin_path = paths.plugin_root.join("test_plugin");
        fs::write(&plugin_path, "#!/bin/bash\necho 'test plugin'\n").unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&plugin_path, fs::Permissions::from_mode(0o755)).unwrap();
        }

        assert!(plugin_exists("test_plugin", &paths));
        assert!(!plugin_exists("nonexistent_plugin", &paths));
    }

    #[test]
    fn test_validate_plugin_name_rejects_path_traversal() {
        assert!(validate_plugin_name("../etc/passwd").is_err());
        assert!(validate_plugin_name("..").is_err());
        assert!(validate_plugin_name("foo/bar").is_err());
        assert!(validate_plugin_name("foo\\bar").is_err());
        assert!(validate_plugin_name("").is_err());
    }

    #[test]
    fn test_validate_plugin_name_allows_valid() {
        assert!(validate_plugin_name("my-plugin").is_ok());
        assert!(validate_plugin_name("plugin_v2").is_ok());
        assert!(validate_plugin_name("deploy.sh").is_ok());
    }

    #[test]
    fn test_plugin_exists_rejects_path_traversal() {
        let temp_dir = TempDir::new().unwrap();
        let paths =
            crate::config::RikuPaths::from_dirs(temp_dir.path().join(".riku"), temp_dir.path());
        fs::create_dir_all(&paths.plugin_root).unwrap();
        assert!(!plugin_exists("../etc/passwd", &paths));
        assert!(!plugin_exists("foo/bar", &paths));
    }
}
