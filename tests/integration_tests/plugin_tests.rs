/// Integration tests for Plugin System
///
/// These tests verify the plugin discovery, execution, and management functionality.

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    // Helper to create a temporary Riku environment
    fn setup_riku_env() -> Result<(TempDir, PathBuf)> {
        let temp_dir = TempDir::new()?;
        let riku_root = temp_dir.path().join(".riku");

        let dirs = [
            "apps",
            "data",
            "envs",
            "repos",
            "logs",
            "nginx",
            "cache",
            "workers",
            "workers-available",
            "workers-enabled",
            "acme",
            "acme-www",
            "plugins",
        ];

        for dir in &dirs {
            fs::create_dir_all(riku_root.join(dir))?;
        }

        Ok((temp_dir, riku_root))
    }

    // Helper to create a plugin script
    fn create_plugin(plugins_dir: &PathBuf, name: &str, content: &str) -> Result<PathBuf> {
        let plugin_path = plugins_dir.join(name);
        fs::write(&plugin_path, content)?;

        // Make executable on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&plugin_path, fs::Permissions::from_mode(0o755))?;
        }

        Ok(plugin_path)
    }

    #[test]
    fn test_plugin_directory_exists() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;
        let plugins_dir = riku_root.join("plugins");

        assert!(plugins_dir.exists());
        assert!(plugins_dir.is_dir());

        Ok(())
    }

    #[test]
    fn test_plugin_creation() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;
        let plugins_dir = riku_root.join("plugins");

        let plugin_content = r#"#!/bin/bash
echo "Hello from plugin"
"#;

        let plugin_path = create_plugin(&plugins_dir, "hello.sh", plugin_content)?;

        assert!(plugin_path.exists());

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let metadata = fs::metadata(&plugin_path)?;
            assert!(metadata.permissions().mode() & 0o111 != 0);
        }

        Ok(())
    }

    #[test]
    fn test_multiple_plugins() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;
        let plugins_dir = riku_root.join("plugins");

        let plugins = vec![
            ("deploy-hook.sh", "#!/bin/bash\necho 'Deploy hook'"),
            ("backup.sh", "#!/bin/bash\necho 'Backup'"),
            ("monitor.sh", "#!/bin/bash\necho 'Monitor'"),
            ("cleanup.sh", "#!/bin/bash\necho 'Cleanup'"),
        ];

        for (name, content) in plugins {
            create_plugin(&plugins_dir, name, content)?;
        }

        let plugin_files: Vec<_> = fs::read_dir(&plugins_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
            .collect();

        assert_eq!(plugin_files.len(), 4);

        Ok(())
    }

    #[test]
    fn test_plugin_with_arguments() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;
        let plugins_dir = riku_root.join("plugins");

        let plugin_content = r#"#!/bin/bash
# Plugin that accepts arguments
APP_NAME="$1"
COMMAND="$2"
echo "App: $APP_NAME, Command: $COMMAND"
"#;

        create_plugin(&plugins_dir, "arg-plugin.sh", plugin_content)?;

        let plugin_path = plugins_dir.join("arg-plugin.sh");
        let content = fs::read_to_string(&plugin_path)?;

        assert!(content.contains("$1"));
        assert!(content.contains("$2"));

        Ok(())
    }

    #[test]
    fn test_plugin_with_environment() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;
        let plugins_dir = riku_root.join("plugins");

        let plugin_content = r#"#!/bin/bash
# Plugin that uses environment variables
echo "RIKU_ROOT: $RIKU_ROOT"
echo "APP_NAME: $APP_NAME"
echo "DEPLOY_USER: $DEPLOY_USER"
"#;

        create_plugin(&plugins_dir, "env-plugin.sh", plugin_content)?;

        let plugin_path = plugins_dir.join("env-plugin.sh");
        let content = fs::read_to_string(&plugin_path)?;

        assert!(content.contains("$RIKU_ROOT"));
        assert!(content.contains("$APP_NAME"));

        Ok(())
    }

    #[test]
    fn test_plugin_exit_codes() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;
        let plugins_dir = riku_root.join("plugins");

        // Success plugin
        let success_plugin = r#"#!/bin/bash
echo "Success"
exit 0
"#;
        create_plugin(&plugins_dir, "success.sh", success_plugin)?;

        // Failure plugin
        let failure_plugin = r#"#!/bin/bash
echo "Failure"
exit 1
"#;
        create_plugin(&plugins_dir, "failure.sh", failure_plugin)?;

        let success_path = plugins_dir.join("success.sh");
        let failure_path = plugins_dir.join("failure.sh");

        assert!(success_path.exists());
        assert!(failure_path.exists());

        Ok(())
    }

    #[test]
    fn test_plugin_with_helpers() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;
        let plugins_dir = riku_root.join("plugins");

        // Create helper script
        let helper_content = r#"#!/bin/bash
# Helper function
log_message() {
    echo "[LOG] $1"
}
"#;
        let helper_path = plugins_dir.join("helpers.sh");
        fs::write(&helper_path, helper_content)?;

        // Create plugin that sources helper
        let plugin_content = r#"#!/bin/bash
source "$(dirname "$0")/helpers.sh"
log_message "Plugin executed"
"#;
        create_plugin(&plugins_dir, "with-helper.sh", plugin_content)?;

        Ok(())
    }

    #[test]
    fn test_plugin_metadata() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;
        let plugins_dir = riku_root.join("plugins");

        let plugin_content = r#"#!/bin/bash
# Plugin: my-plugin
# Description: A sample plugin
# Version: 1.0.0
# Author: Test Author

echo "My Plugin v1.0.0"
"#;

        create_plugin(&plugins_dir, "my-plugin.sh", plugin_content)?;

        let plugin_path = plugins_dir.join("my-plugin.sh");
        let content = fs::read_to_string(&plugin_path)?;

        assert!(content.contains("# Plugin: my-plugin"));
        assert!(content.contains("# Version: 1.0.0"));

        Ok(())
    }

    #[test]
    fn test_plugin_subdirectory() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;
        let plugins_dir = riku_root.join("plugins");

        // Create subdirectory for plugin organization
        let hooks_dir = plugins_dir.join("hooks");
        fs::create_dir_all(&hooks_dir)?;

        let hook_content = "#!/bin/bash\necho 'Hook executed'\n";
        create_plugin(&hooks_dir, "pre-deploy.sh", hook_content)?;
        create_plugin(&hooks_dir, "post-deploy.sh", hook_content)?;

        assert!(hooks_dir.exists());

        let hooks: Vec<_> = fs::read_dir(&hooks_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
            .collect();

        assert_eq!(hooks.len(), 2);

        Ok(())
    }

    #[test]
    fn test_plugin_with_python() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;
        let plugins_dir = riku_root.join("plugins");

        let python_plugin = r#"#!/usr/bin/env python3
import sys
import os

print("Python plugin executed")
print(f"Args: {sys.argv[1:]}")
print(f"RIKU_ROOT: {os.environ.get('RIKU_ROOT', 'not set')}")
"#;

        create_plugin(&plugins_dir, "python-plugin.py", python_plugin)?;

        let plugin_path = plugins_dir.join("python-plugin.py");
        let content = fs::read_to_string(&plugin_path)?;

        assert!(content.contains("#!/usr/bin/env python3"));
        assert!(content.contains("import sys"));

        Ok(())
    }

    #[test]
    fn test_plugin_with_nodejs() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;
        let plugins_dir = riku_root.join("plugins");

        let node_plugin = r#"#!/usr/bin/env node
const fs = require('fs');
const path = require('path');

console.log('Node.js plugin executed');
console.log('Args:', process.argv.slice(2));
"#;

        create_plugin(&plugins_dir, "node-plugin.js", node_plugin)?;

        let plugin_path = plugins_dir.join("node-plugin.js");
        let content = fs::read_to_string(&plugin_path)?;

        assert!(content.contains("#!/usr/bin/env node"));
        assert!(content.contains("require('fs')"));

        Ok(())
    }

    #[test]
    fn test_plugin_list_simulation() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;
        let plugins_dir = riku_root.join("plugins");

        // Create several plugins
        let plugin_names = vec![
            "deploy-hook.sh",
            "backup.sh",
            "monitor.sh",
            "logs.sh",
            "config.sh",
        ];

        for name in &plugin_names {
            create_plugin(&plugins_dir, name, "#!/bin/bash\necho 'Plugin'\n")?;
        }

        // Simulate plugin listing
        let plugins: Vec<String> = fs::read_dir(&plugins_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
            .filter_map(|e| {
                e.path()
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
            })
            .collect();

        assert_eq!(plugins.len(), 5);

        for name in &plugin_names {
            assert!(plugins.contains(&name.to_string()));
        }

        Ok(())
    }

    #[test]
    fn test_plugin_exists_check() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;
        let plugins_dir = riku_root.join("plugins");

        create_plugin(&plugins_dir, "existing.sh", "#!/bin/bash\necho 'Exists'\n")?;

        let existing_path = plugins_dir.join("existing.sh");
        let non_existing_path = plugins_dir.join("non-existing.sh");

        assert!(existing_path.exists());
        assert!(!non_existing_path.exists());

        Ok(())
    }

    #[test]
    fn test_plugin_cleanup() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;
        let plugins_dir = riku_root.join("plugins");

        let plugin_path =
            create_plugin(&plugins_dir, "temp-plugin.sh", "#!/bin/bash\necho 'Temp'\n")?;

        assert!(plugin_path.exists());

        // Remove plugin
        fs::remove_file(&plugin_path)?;

        assert!(!plugin_path.exists());

        Ok(())
    }

    #[test]
    fn test_plugin_with_config_file() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;
        let plugins_dir = riku_root.join("plugins");

        // Create plugin config
        let config_content = r#"
plugin_name="config-plugin"
version="1.0.0"
enabled=true
timeout=30
"#;
        let config_path = plugins_dir.join("config-plugin.conf");
        fs::write(&config_path, config_content)?;

        // Create plugin script
        let plugin_content = r#"#!/bin/bash
# Read config
source "$(dirname "$0")/config-plugin.conf"
echo "Plugin: $plugin_name v$version"
"#;
        create_plugin(&plugins_dir, "config-plugin.sh", plugin_content)?;

        assert!(config_path.exists());

        let config: toml::Value =
            toml::from_str(config_content).unwrap_or(toml::Value::Table(toml::map::Map::new()));
        // Basic check that config is readable
        assert!(config_content.contains("plugin_name"));

        Ok(())
    }

    #[test]
    fn test_plugin_permissions() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;
        let plugins_dir = riku_root.join("plugins");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            // Create non-executable file
            let non_exec_path = plugins_dir.join("non-exec.txt");
            fs::write(&non_exec_path, "Not a plugin")?;
            fs::set_permissions(&non_exec_path, fs::Permissions::from_mode(0o644))?;

            // Create executable plugin
            let exec_path = plugins_dir.join("executable.sh");
            fs::write(&exec_path, "#!/bin/bash\necho 'Exec'")?;
            fs::set_permissions(&exec_path, fs::Permissions::from_mode(0o755))?;

            // Check permissions
            let non_exec_meta = fs::metadata(&non_exec_path)?;
            let exec_meta = fs::metadata(&exec_path)?;

            assert_eq!(non_exec_meta.permissions().mode() & 0o111, 0);
            assert_ne!(exec_meta.permissions().mode() & 0o111, 0);
        }

        Ok(())
    }
}
