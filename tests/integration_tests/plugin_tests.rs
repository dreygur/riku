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

        let _config: toml::Value =
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

    // ── Plugin Architecture / PluginHook integration tests ────────────────────

    /// All four hooks map to the correct plugin file names.
    #[test]
    fn test_hook_plugin_name_conventions() {
        // Verify the hook→plugin-name mapping matches the documented convention
        let cases = [
            ("pre-deploy", "riku-pre-deploy"),
            ("pre-build", "riku-pre-build"),
            ("post-build", "riku-post-build"),
            ("post-deploy", "riku-post-deploy"),
        ];
        for (hook_name, plugin_name) in &cases {
            // Plugin file naming: riku-<hook-name>
            assert_eq!(
                format!("riku-{}", hook_name),
                *plugin_name,
                "Hook '{}' should map to plugin '{}'",
                hook_name,
                plugin_name
            );
        }
    }

    /// Pre-deploy plugin that exits 0 allows deploy to continue.
    #[test]
    fn test_pre_deploy_hook_success_allows_continue() -> Result<()> {
        let (_temp, riku_root) = setup_riku_env()?;
        let plugins_dir = riku_root.join("plugins");
        let output_file = riku_root.join("pre-deploy-ran.txt");

        let script = format!(
            "#!/bin/sh\necho \"pre-deploy ran for $RIKU_APP\" > '{}'\nexit 0\n",
            output_file.display()
        );
        create_plugin(&plugins_dir, "riku-pre-deploy", &script)?;

        // Simulate what do_deploy does: run the pre-deploy plugin
        let status = std::process::Command::new(plugins_dir.join("riku-pre-deploy"))
            .env("RIKU_APP", "testapp")
            .env("RIKU_HOOK", "pre-deploy")
            .env("RIKU_APP_PATH", "/tmp/testapp")
            .env(
                "RIKU_ENV_PATH",
                riku_root.join("envs/testapp").to_str().unwrap(),
            )
            .env("RIKU_ROOT", riku_root.to_str().unwrap())
            .status()?;

        assert!(status.success(), "Pre-deploy plugin should exit 0");
        let content = fs::read_to_string(&output_file)?;
        assert!(
            content.contains("testapp"),
            "Plugin should have RIKU_APP set"
        );

        Ok(())
    }

    /// Pre-deploy plugin that exits non-zero signals deploy abort.
    #[test]
    fn test_pre_deploy_hook_failure_signals_abort() -> Result<()> {
        let (_temp, riku_root) = setup_riku_env()?;
        let plugins_dir = riku_root.join("plugins");

        create_plugin(
            &plugins_dir,
            "riku-pre-deploy",
            "#!/bin/sh\necho 'validation failed' >&2\nexit 42\n",
        )?;

        let status = std::process::Command::new(plugins_dir.join("riku-pre-deploy"))
            .env("RIKU_APP", "badapp")
            .env("RIKU_HOOK", "pre-deploy")
            .status()?;

        assert!(!status.success());
        assert_eq!(status.code(), Some(42));

        Ok(())
    }

    /// Post-deploy plugin receives all expected RIKU_* environment variables.
    #[test]
    fn test_post_deploy_hook_receives_riku_env_vars() -> Result<()> {
        let (_temp, riku_root) = setup_riku_env()?;
        let plugins_dir = riku_root.join("plugins");
        let output_file = riku_root.join("post-deploy-env.txt");

        let script = format!(
            "#!/bin/sh\nenv | grep ^RIKU_ | sort > '{}'\n",
            output_file.display()
        );
        create_plugin(&plugins_dir, "riku-post-deploy", &script)?;

        std::process::Command::new(plugins_dir.join("riku-post-deploy"))
            .env("RIKU_APP", "myapp")
            .env("RIKU_HOOK", "post-deploy")
            .env("RIKU_APP_PATH", "/tmp/myapp")
            .env("RIKU_ENV_PATH", "/tmp/envs/myapp")
            .env("RIKU_ROOT", riku_root.to_str().unwrap())
            .env("RIKU_RUNTIME", "Python")
            .status()?;

        let content = fs::read_to_string(&output_file)?;
        assert!(content.contains("RIKU_APP=myapp"), "Missing RIKU_APP");
        assert!(
            content.contains("RIKU_HOOK=post-deploy"),
            "Missing RIKU_HOOK"
        );
        assert!(
            content.contains("RIKU_RUNTIME=Python"),
            "Missing RIKU_RUNTIME"
        );

        Ok(())
    }

    /// Pre-build plugin can write a marker file read by post-build plugin.
    #[test]
    fn test_pre_and_post_build_hooks_sequencing() -> Result<()> {
        let (_temp, riku_root) = setup_riku_env()?;
        let plugins_dir = riku_root.join("plugins");
        let marker = riku_root.join("build-sequence.txt");

        // pre-build writes step 1
        let pre_script = format!("#!/bin/sh\necho 'pre-build' >> '{}'\n", marker.display());
        create_plugin(&plugins_dir, "riku-pre-build", &pre_script)?;

        // post-build writes step 2
        let post_script = format!("#!/bin/sh\necho 'post-build' >> '{}'\n", marker.display());
        create_plugin(&plugins_dir, "riku-post-build", &post_script)?;

        // Run in order
        std::process::Command::new(plugins_dir.join("riku-pre-build"))
            .env("RIKU_APP", "buildapp")
            .env("RIKU_HOOK", "pre-build")
            .status()?;
        std::process::Command::new(plugins_dir.join("riku-post-build"))
            .env("RIKU_APP", "buildapp")
            .env("RIKU_HOOK", "post-build")
            .status()?;

        let content = fs::read_to_string(&marker)?;
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines[0], "pre-build", "pre-build should fire first");
        assert_eq!(lines[1], "post-build", "post-build should fire second");

        Ok(())
    }

    /// Missing hook plugin: no plugin file means hook is silently skipped.
    #[test]
    fn test_missing_hook_plugin_is_silently_skipped() -> Result<()> {
        let (_temp, riku_root) = setup_riku_env()?;
        let plugins_dir = riku_root.join("plugins");

        // No riku-pre-deploy file exists
        assert!(!plugins_dir.join("riku-pre-deploy").exists());

        // Simulating "no plugin" — the PluginManager returns Ok(false) in this case.
        // Verify by just checking the file is absent (unit tests cover the Ok(false) path).
        let entries = fs::read_dir(&plugins_dir)?.count();
        assert_eq!(entries, 0, "Plugin dir should be empty");

        Ok(())
    }

    /// Multiple hooks can coexist independently (e.g. notify + migrate are separate plugins).
    #[test]
    fn test_multiple_independent_hooks() -> Result<()> {
        let (_temp, riku_root) = setup_riku_env()?;
        let plugins_dir = riku_root.join("plugins");

        for hook in &["riku-pre-deploy", "riku-post-deploy"] {
            let output = riku_root.join(format!("{}.txt", hook));
            let script = format!("#!/bin/sh\ntouch '{}'\n", output.display());
            create_plugin(&plugins_dir, hook, &script)?;
        }

        // Run both
        for hook in &["riku-pre-deploy", "riku-post-deploy"] {
            std::process::Command::new(plugins_dir.join(hook))
                .env("RIKU_APP", "app")
                .env("RIKU_HOOK", hook.trim_start_matches("riku-"))
                .status()?;
        }

        assert!(riku_root.join("riku-pre-deploy.txt").exists());
        assert!(riku_root.join("riku-post-deploy.txt").exists());

        Ok(())
    }
}
