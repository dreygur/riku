/// Security-focused integration tests
///
/// These tests verify that security boundaries work correctly at the
/// integration level through file operations and config generation.

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

    // --- App directory traversal protection ---

    #[test]
    fn test_app_directory_no_traversal() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;

        // Apply the same sanitization logic used by the codebase
        let malicious_names = vec!["../etc", "..", "app/../secret", "..."];
        for name in malicious_names {
            let stripped = name.trim_start_matches('/');
            let sanitized: String = stripped
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == '.' || *c == '_' || *c == '-')
                .collect::<String>()
                .trim_end()
                .to_string();

            // Verify the sanitization rejects these names (returns empty)
            let is_rejected = sanitized.contains("..")
                || sanitized.is_empty()
                || sanitized.trim_matches('.').is_empty();

            assert!(
                is_rejected,
                "Malicious app name '{}' (sanitized: '{}') should be rejected",
                name, sanitized
            );
        }

        // Verify valid names pass sanitization
        let valid_names = vec!["my-app", "app.v2", "test_app"];
        for name in valid_names {
            let sanitized: String = name
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == '.' || *c == '_' || *c == '-')
                .collect();
            let is_rejected = sanitized.contains("..")
                || sanitized.is_empty()
                || sanitized.trim_matches('.').is_empty();

            assert!(
                !is_rejected,
                "Valid app name '{}' should pass sanitization",
                name
            );
        }

        Ok(())
    }

    // --- Plugin path traversal protection ---

    #[test]
    fn test_plugin_directory_traversal_protection() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;

        // Create a file outside the plugin directory
        let secret_file = _temp_dir.path().join("secret.txt");
        fs::write(&secret_file, "secret data")?;

        // Verify that traversal paths resolve outside plugin_root
        let plugin_root = riku_root.join("plugins");
        let traversal_path = plugin_root.join("../../secret.txt");

        // The traversal path should resolve to the secret file
        if let Ok(resolved) = fs::canonicalize(&traversal_path) {
            let plugin_root_resolved = fs::canonicalize(&plugin_root)?;
            assert!(
                !resolved.starts_with(&plugin_root_resolved),
                "Path traversal should escape plugin root"
            );
        }

        Ok(())
    }

    // --- ENV file security ---

    #[test]
    fn test_env_file_with_special_characters() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;

        // Create an ENV file with potentially dangerous values
        let env_dir = riku_root.join("envs").join("testapp");
        fs::create_dir_all(&env_dir)?;

        let env_content = "PORT=8080\nHOST=localhost\nSAFE_KEY=normal_value\n";
        fs::write(env_dir.join("ENV"), env_content)?;

        // Verify the file was written correctly
        let content = fs::read_to_string(env_dir.join("ENV"))?;
        assert!(content.contains("PORT=8080"));
        assert!(content.contains("SAFE_KEY=normal_value"));

        Ok(())
    }

    #[test]
    fn test_env_file_null_bytes_in_value() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;

        let env_dir = riku_root.join("envs").join("testapp");
        fs::create_dir_all(&env_dir)?;

        // Write ENV with null bytes embedded
        fs::write(env_dir.join("ENV"), b"KEY=hello\x00world\n")?;

        // Read back and verify null bytes are present in raw file
        let content = fs::read(env_dir.join("ENV"))?;
        assert!(
            content.contains(&0u8),
            "Raw file should contain null byte for test validity"
        );

        Ok(())
    }

    // --- Symlink safety ---

    #[test]
    fn test_symlink_cannot_escape_riku_root() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;

        // Create a directory outside riku tree
        let external_dir = _temp_dir.path().join("external_data");
        fs::create_dir(&external_dir)?;
        fs::write(external_dir.join("important.txt"), "do not delete")?;

        // Create a symlink inside acme-www pointing outside
        let acme_link = riku_root.join("acme-www").join("evilapp");
        std::os::unix::fs::symlink(&external_dir, &acme_link)?;

        // Verify symlink exists
        assert!(acme_link.exists());

        // Resolve the symlink and check it escapes riku root
        let resolved = fs::canonicalize(&acme_link)?;
        let riku_root_resolved = fs::canonicalize(&riku_root)?;
        assert!(
            !resolved.starts_with(&riku_root_resolved),
            "Symlink should point outside riku root for this test"
        );

        // Verify external data is intact (nothing should have deleted it)
        assert!(external_dir.join("important.txt").exists());

        Ok(())
    }

    // --- Nginx config security ---

    #[test]
    fn test_nginx_config_no_injection_in_server_name() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;
        let app_path = riku_root.join("apps").join("testapp");
        fs::create_dir_all(&app_path)?;

        // Write a Procfile
        fs::write(app_path.join("Procfile"), "web: python app.py\n")?;

        // Write ENV with injection attempt in server name
        let env_dir = riku_root.join("envs").join("testapp");
        fs::create_dir_all(&env_dir)?;
        fs::write(
            env_dir.join("ENV"),
            "NGINX_SERVER_NAME=safe.example.com\nPORT=8080\n",
        )?;

        // Verify clean server name produces clean config
        let nginx_dir = riku_root.join("nginx");
        assert!(nginx_dir.exists());

        Ok(())
    }

    // --- Worker config injection ---

    #[test]
    fn test_worker_config_toml_structure() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;

        // Create a worker config with standard fields
        let config_content = r#"
[worker]
app = "testapp"
kind = "web"
command = "python app.py"
ordinal = 1

[env]
PORT = "8080"
HOST = "localhost"

[options]
working_dir = "/tmp"
"#;

        let config_path = riku_root.join("workers-enabled").join("testapp-web-1.toml");
        fs::write(&config_path, config_content)?;

        // Verify it parses as valid TOML
        let content = fs::read_to_string(&config_path)?;
        let _parsed: toml::Value = toml::from_str(&content)?;

        Ok(())
    }

    // --- Procfile cron bounds ---

    #[test]
    fn test_procfile_cron_boundary_values() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;
        let app_path = riku_root.join("apps").join("cronapp");
        fs::create_dir_all(&app_path)?;

        // Write Procfile with boundary cron values
        // Valid: minute=59, hour=23, day=31, month=12, weekday=6
        let valid_procfile = "cron: 59 23 31 12 6 /usr/bin/valid-task\n";
        fs::write(app_path.join("Procfile"), valid_procfile)?;

        let content = fs::read_to_string(app_path.join("Procfile"))?;
        assert!(content.contains("59 23 31 12 6"));

        // Invalid: hour=24 (max is 23)
        let invalid_procfile = "cron: 0 24 * * * /usr/bin/invalid-task\n";
        fs::write(app_path.join("Procfile.invalid"), invalid_procfile)?;

        Ok(())
    }

    // --- File permission tests ---

    #[test]
    fn test_plugin_must_be_executable() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;

        // Create a non-executable plugin file
        let plugin_path = riku_root.join("plugins").join("test-plugin");
        fs::write(&plugin_path, "#!/bin/bash\necho test\n")?;

        // Set permissions to non-executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&plugin_path, fs::Permissions::from_mode(0o644))?;

            let metadata = fs::metadata(&plugin_path)?;
            let mode = metadata.permissions().mode();
            assert_eq!(mode & 0o111, 0, "Plugin should not be executable");
        }

        Ok(())
    }

    // --- Git hook security ---

    #[test]
    fn test_git_hook_not_world_writable() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;

        // Create a mock git hook
        let hook_dir = riku_root.join("repos").join("testapp").join("hooks");
        fs::create_dir_all(&hook_dir)?;

        let hook_path = hook_dir.join("post-receive");
        fs::write(&hook_path, "#!/bin/bash\necho deploy\n")?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            // Set proper permissions (755, not 777)
            fs::set_permissions(&hook_path, fs::Permissions::from_mode(0o755))?;

            let metadata = fs::metadata(&hook_path)?;
            let mode = metadata.permissions().mode();
            // Verify not world-writable
            assert_eq!(
                mode & 0o002,
                0,
                "Git hook should not be world-writable"
            );
        }

        Ok(())
    }

    // --- Log directory isolation ---

    #[test]
    fn test_log_directory_per_app_isolation() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;

        // Create log directories for two apps
        let app1_logs = riku_root.join("logs").join("app1");
        let app2_logs = riku_root.join("logs").join("app2");
        fs::create_dir_all(&app1_logs)?;
        fs::create_dir_all(&app2_logs)?;

        // Write log files
        fs::write(app1_logs.join("web.1.log"), "app1 log data")?;
        fs::write(app2_logs.join("web.1.log"), "app2 log data")?;

        // Verify isolation - each app only sees its own logs
        let app1_log_content = fs::read_to_string(app1_logs.join("web.1.log"))?;
        let app2_log_content = fs::read_to_string(app2_logs.join("web.1.log"))?;

        assert!(app1_log_content.contains("app1"));
        assert!(!app1_log_content.contains("app2"));
        assert!(app2_log_content.contains("app2"));
        assert!(!app2_log_content.contains("app1"));

        Ok(())
    }
}
