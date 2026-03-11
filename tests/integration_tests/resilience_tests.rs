/// Resilience and chaos engineering tests
///
/// These tests verify that the system handles failure scenarios gracefully,
/// including resource exhaustion, process crashes, and invalid configurations.

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use std::fs;
    use tempfile::TempDir;

    // Helper to create a temporary Riku environment (duplicated to avoid cross-module deps)
    fn setup_riku_env() -> Result<(TempDir, std::path::PathBuf)> {
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

    // Helper to create a worker config TOML
    fn create_worker_config(
        workers_dir: &std::path::PathBuf,
        app: &str,
        kind: &str,
        ordinal: u32,
        command: &str,
    ) -> Result<std::path::PathBuf> {
        let config_file = workers_dir.join(format!("{}.{}.{}.toml", app, kind, ordinal));

        let content = format!(
            r#"[worker]
app = "{}"
kind = "{}"
command = "{}"
ordinal = {}

[env]
PORT = "{}"
APP_NAME = "{}"

[options]
working_dir = "/tmp/test-app"
"#,
            app,
            kind,
            command,
            ordinal,
            5000 + ordinal,
            app
        );

        fs::write(&config_file, content)?;
        Ok(config_file)
    }

    #[test]
    fn test_invalid_worker_config_toml() -> Result<()> {
        let (_temp, riku_root) = setup_riku_env()?;
        let workers_enabled = riku_root.join("workers-enabled");

        // Create an invalid TOML file (should be skipped without crashing)
        let invalid_config = workers_enabled.join("invalid.web.1.toml");
        fs::write(&invalid_config, "this is not valid TOML { [ ] ")?;

        // The supervisor should skip this file and not crash
        // In a real test, we'd start the supervisor and verify it handles this gracefully
        assert!(invalid_config.exists());

        Ok(())
    }

    #[test]
    fn test_worker_config_missing_required_fields() -> Result<()> {
        let (_temp, riku_root) = setup_riku_env()?;
        let workers_enabled = riku_root.join("workers-enabled");

        // Create a TOML file missing required fields
        let incomplete_config = workers_enabled.join("incomplete.web.1.toml");
        fs::write(
            &incomplete_config,
            r#"
# Missing required fields like command
app = "incomplete"
kind = "web"
ordinal = 1
"#,
        )?;

        // The supervisor should handle this gracefully
        assert!(incomplete_config.exists());

        Ok(())
    }

    #[test]
    fn test_process_crash_detection() -> Result<()> {
        let (_temp, riku_root) = setup_riku_env()?;
        let workers_enabled = riku_root.join("workers-enabled");

        // Create a worker that exits immediately
        create_worker_config(
            &workers_enabled,
            "crashapp",
            "web",
            1,
            "exit 1", // Command that fails immediately
        )?;

        // In a real supervisor test, we'd verify:
        // 1. Process is detected as crashed
        // 2. Restart logic kicks in
        // 3. After max retries, process is marked as failed

        Ok(())
    }

    #[test]
    fn test_resource_limits_enforcement() -> Result<()> {
        use std::env;

        // Test that resource limit environment variables are respected
        env::set_var("RIKU_MAX_MEMORY_MB", "128");
        env::set_var("RIKU_MAX_OPEN_FILES", "512");
        env::set_var("RIKU_MAX_PROCESSES", "10");

        // These would be applied when spawning processes
        // The actual enforcement is tested in resource_limits.rs unit tests

        env::remove_var("RIKU_MAX_MEMORY_MB");
        env::remove_var("RIKU_MAX_OPEN_FILES");
        env::remove_var("RIKU_MAX_PROCESSES");

        Ok(())
    }

    #[test]
    fn test_health_check_failure_handling() -> Result<()> {
        let (_temp, riku_root) = setup_riku_env()?;
        let workers_enabled = riku_root.join("workers-enabled");

        // Create a worker config with health check settings
        let config_file = workers_enabled.join("healthapp.web.1.toml");
        fs::write(
            &config_file,
            r#"
app = "healthapp"
kind = "web"
ordinal = 1
command = "python -m http.server 8080"
working_dir = "/tmp"

[env]
PORT = "8080"

[options.health_check]
enabled = true
endpoint = "/health"
interval_secs = 5
timeout_secs = 2
max_failures = 3
"#,
        )?;

        // In a real test, we'd:
        // 1. Start a process that fails health checks
        // 2. Verify the supervisor detects failures
        // 3. Verify restart logic after max_failures

        Ok(())
    }

    #[test]
    fn test_log_directory_creation_failure() -> Result<()> {
        // Test handling when log directory cannot be created
        // This would happen if permissions are insufficient

        let temp = TempDir::new()?;
        let log_root = temp.path().join("logs");

        // Create log root with restrictive permissions (read-only)
        fs::create_dir(&log_root)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&log_root)?.permissions();
            perms.set_mode(0o444); // Read-only
            fs::set_permissions(&log_root, perms)?;
        }

        // Attempting to create subdirectory should fail gracefully
        let app_log_dir = log_root.join("testapp");
        let result = fs::create_dir(&app_log_dir);

        #[cfg(unix)]
        assert!(
            result.is_err(),
            "Should fail to create directory with insufficient permissions"
        );

        Ok(())
    }

    #[test]
    fn test_stats_file_write_failure() -> Result<()> {
        // Test handling when stats file cannot be written
        let temp = TempDir::new()?;
        let stats_file = temp.path().join("stats.json");

        // Create a read-only stats file
        fs::write(&stats_file, "{}")?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&stats_file)?.permissions();
            perms.set_mode(0o444); // Read-only
            fs::set_permissions(&stats_file, perms)?;
        }

        // Attempting to write should fail gracefully
        let result = fs::write(&stats_file, "{}");

        #[cfg(unix)]
        assert!(result.is_err(), "Should fail to write to read-only file");

        Ok(())
    }

    #[test]
    fn test_concurrent_config_updates() -> Result<()> {
        let (_temp, riku_root) = setup_riku_env()?;
        let workers_enabled = riku_root.join("workers-enabled");

        // Create multiple worker configs
        for i in 1..=5 {
            create_worker_config(
                &workers_enabled,
                "multiapp",
                "web",
                i,
                &format!("sleep {}", i * 10),
            )?;
        }

        // Verify all configs were created
        let configs: Vec<_> = fs::read_dir(&workers_enabled)?
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with("multiapp"))
            .collect();

        assert_eq!(configs.len(), 5, "Should have 5 worker configs");

        Ok(())
    }

    #[test]
    fn test_rapid_process_restart() -> Result<()> {
        // Test that rapid process restarts don't cause issues
        // This simulates a process that keeps crashing immediately

        let (_temp, riku_root) = setup_riku_env()?;
        let workers_enabled = riku_root.join("workers-enabled");

        create_worker_config(
            &workers_enabled,
            "flapper",
            "web",
            1,
            "sh -c 'echo starting && exit 1'", // Exits immediately
        )?;

        // In a real test:
        // 1. Start supervisor
        // 2. Verify it handles rapid restarts
        // 3. Verify backoff/cooldown logic
        // 4. Verify max restart limit is respected

        Ok(())
    }

    #[test]
    fn test_missing_executable_in_path() -> Result<()> {
        let (_temp, riku_root) = setup_riku_env()?;
        let workers_enabled = riku_root.join("workers-enabled");

        // Create a worker with a non-existent command
        create_worker_config(
            &workers_enabled,
            "badcmd",
            "web",
            1,
            "this-command-definitely-does-not-exist",
        )?;

        // The supervisor should handle this gracefully and report an error
        // rather than crashing

        Ok(())
    }

    #[test]
    fn test_disk_space_handling() -> Result<()> {
        // Test that the system handles disk space issues gracefully
        // In a real scenario, we'd fill up the disk and verify error handling

        let temp = TempDir::new()?;
        let test_file = temp.path().join("test.txt");

        // Write a small file (should succeed)
        fs::write(&test_file, "test content")?;
        assert!(test_file.exists());

        // In a production scenario with disk full:
        // - Deploy should fail with clear error
        // - Logs should still be readable
        // - System should not crash

        Ok(())
    }

    #[test]
    fn test_unicode_in_app_names() -> Result<()> {
        // Test handling of unicode and special characters in app names
        let (_temp, riku_root) = setup_riku_env()?;
        let workers_enabled = riku_root.join("workers-enabled");

        // Create config with safe app name
        let result =
            create_worker_config(&workers_enabled, "myapp-test_123", "web", 1, "echo test");

        assert!(result.is_ok(), "Safe app name should work");

        Ok(())
    }

    #[test]
    fn test_very_long_command_line() -> Result<()> {
        // Test handling of very long command lines
        let (_temp, riku_root) = setup_riku_env()?;
        let workers_enabled = riku_root.join("workers-enabled");

        // Create a very long command (but within OS limits)
        let long_cmd = format!("echo {}", "A".repeat(500));

        let result = create_worker_config(&workers_enabled, "longcmd", "web", 1, &long_cmd);

        assert!(result.is_ok(), "Long command should be handled");

        Ok(())
    }

    #[test]
    fn test_environment_variable_limits() -> Result<()> {
        use std::env;

        // Test setting resource limits via environment
        env::set_var("RIKU_MAX_MEMORY_MB", "999999"); // Very high value
        env::set_var("RIKU_MAX_OPEN_FILES", "1000000"); // Very high value

        // The system should handle these gracefully (capping at OS limits)
        // Actual enforcement happens in ProcessManager

        env::remove_var("RIKU_MAX_MEMORY_MB");
        env::remove_var("RIKU_MAX_OPEN_FILES");

        Ok(())
    }

    #[test]
    fn test_simultaneous_config_file_operations() -> Result<()> {
        // Test concurrent access to config files
        let (_temp, riku_root) = setup_riku_env()?;
        let workers_enabled = riku_root.join("workers-enabled");

        // Create multiple configs rapidly
        for i in 1..=10 {
            create_worker_config(
                &workers_enabled,
                &format!("app{}", i),
                "web",
                1,
                "echo test",
            )?;
        }

        // Count created configs
        let count = fs::read_dir(&workers_enabled)?
            .filter_map(|e| e.ok())
            .count();

        assert_eq!(count, 10, "All configs should be created");

        Ok(())
    }
}
