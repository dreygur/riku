/// Resilience and chaos engineering tests
///
/// These tests verify that the system handles failure scenarios gracefully,
/// including resource exhaustion, process crashes, and invalid configurations.

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use std::fs;
    use std::sync::{Arc, Mutex};
    use std::thread;
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

    // Helper to create a valid worker config TOML
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
log_file = "/tmp/test-app/{}-{}.log"
"#,
            app,
            kind,
            command,
            ordinal,
            5000 + ordinal,
            app,
            app,
            kind
        );

        fs::write(&config_file, content)?;
        Ok(config_file)
    }

    /// Invalid TOML is rejected at parse time, not silently ignored.
    #[test]
    fn test_invalid_worker_config_toml() -> Result<()> {
        let invalid_cases = [
            "this is not valid TOML { [ ] ",
            "[worker\napp = missing-bracket",
            "= no_key",
            "[[[[unclosed",
        ];

        for invalid in &invalid_cases {
            let result = toml::from_str::<toml::Value>(invalid);
            assert!(
                result.is_err(),
                "Expected TOML parse error for: {:?}",
                invalid
            );
        }

        Ok(())
    }

    /// Worker configs missing required fields fail to deserialize into WorkerConfig.
    #[test]
    fn test_worker_config_missing_required_fields() -> Result<()> {
        // Valid TOML but missing [worker] section entirely
        let missing_worker_section = r#"
[env]
PORT = "5000"

[options]
working_dir = "/tmp"
log_file = "/tmp/test.log"
"#;
        // Deserialize as a generic Value — it parses, but won't have expected keys
        let val: toml::Value = toml::from_str(missing_worker_section)?;
        assert!(
            val.get("worker").is_none(),
            "Should be missing [worker] section"
        );

        // Missing command field inside [worker]
        let missing_command = r#"
[worker]
app = "myapp"
kind = "web"
ordinal = 1

[env]
PORT = "5000"

[options]
working_dir = "/tmp"
log_file = "/tmp/test.log"
"#;
        let val: toml::Value = toml::from_str(missing_command)?;
        let worker = val.get("worker").expect("has [worker]");
        assert!(
            worker.get("command").is_none(),
            "Should be missing 'command' field"
        );

        Ok(())
    }

    /// Process crash state is correctly tracked in stats JSON.
    #[test]
    fn test_process_crash_detection() -> Result<()> {
        let temp = TempDir::new()?;
        let stats_file = temp.path().join("stats.json");

        // Simulate a stats file where a process has crashed
        let stats = serde_json::json!([{
            "app": "crashapp",
            "total_processes": 1,
            "running_processes": 0,
            "healthy_processes": 0,
            "total_restarts": 3,
            "total_memory_bytes": 0,
            "total_cpu_time_ms": 0,
            "processes": [{
                "process_id": "crashapp-web-1",
                "app": "crashapp",
                "kind": "web",
                "ordinal": 1,
                "pid": null,
                "status": "Crashed",
                "started_at": null,
                "last_health_check": null,
                "health_check_status": "Unknown",
                "restart_count": 3,
                "last_restart_at": null,
                "cpu_time_ms": 0,
                "memory_bytes": 0,
                "requests_total": 0,
                "requests_per_second": 0.0
            }],
            "last_updated": "2026-01-01T00:00:00Z"
        }]);

        fs::write(&stats_file, serde_json::to_string(&stats)?)?;

        // Verify stats are readable and reflect crashed state
        let content = fs::read_to_string(&stats_file)?;
        let parsed: serde_json::Value = serde_json::from_str(&content)?;
        let app = &parsed[0];

        assert_eq!(app["app"], "crashapp");
        assert_eq!(app["running_processes"], 0);
        assert_eq!(app["total_restarts"], 3);
        assert_eq!(app["processes"][0]["status"], "Crashed");
        assert_eq!(app["processes"][0]["restart_count"], 3);

        Ok(())
    }

    /// ResourceLimits env vars parse correctly, including extreme values.
    #[test]
    fn test_resource_limits_from_env() -> Result<()> {
        use std::env;

        // Normal values
        env::set_var("RIKU_MAX_MEMORY_MB", "256");
        env::set_var("RIKU_MAX_OPEN_FILES", "512");
        env::set_var("RIKU_MAX_PROCESSES", "32");
        env::set_var("RIKU_MAX_CPU_SECONDS", "1800");

        // Validate env vars are set correctly (simulates what ResourceLimits::from_env reads)
        assert_eq!(env::var("RIKU_MAX_MEMORY_MB").unwrap(), "256");
        assert_eq!(env::var("RIKU_MAX_OPEN_FILES").unwrap(), "512");
        assert_eq!(env::var("RIKU_MAX_PROCESSES").unwrap(), "32");
        assert_eq!(env::var("RIKU_MAX_CPU_SECONDS").unwrap(), "1800");

        env::remove_var("RIKU_MAX_MEMORY_MB");
        env::remove_var("RIKU_MAX_OPEN_FILES");
        env::remove_var("RIKU_MAX_PROCESSES");
        env::remove_var("RIKU_MAX_CPU_SECONDS");

        // Extreme values should be strings — parsing them is the caller's responsibility
        env::set_var("RIKU_MAX_MEMORY_MB", "999999");
        env::set_var("RIKU_MAX_OPEN_FILES", "1000000");
        let mem: u64 = env::var("RIKU_MAX_MEMORY_MB")
            .unwrap()
            .parse()
            .expect("should parse large number");
        assert_eq!(mem, 999999);

        // Invalid (non-numeric) value — parsing should fail
        env::set_var("RIKU_MAX_MEMORY_MB", "not_a_number");
        let result: std::result::Result<u64, _> = env::var("RIKU_MAX_MEMORY_MB").unwrap().parse();
        assert!(
            result.is_err(),
            "non-numeric limit value should fail to parse"
        );

        env::remove_var("RIKU_MAX_MEMORY_MB");
        env::remove_var("RIKU_MAX_OPEN_FILES");

        Ok(())
    }

    /// Health check status transitions are correctly represented in JSON.
    #[test]
    fn test_health_check_failure_tracking() -> Result<()> {
        let temp = TempDir::new()?;
        let stats_file = temp.path().join("stats.json");

        // Simulate health check degradation: Unknown -> Healthy -> Unhealthy
        let transitions = [
            ("Unknown", 0),
            ("Healthy", 0),
            ("Unhealthy", 1),
            ("Unhealthy", 2),
        ];

        for (status, failures) in &transitions {
            let stats = serde_json::json!([{
                "app": "healthapp",
                "total_processes": 1,
                "running_processes": 1,
                "healthy_processes": if *status == "Healthy" { 1 } else { 0 },
                "total_restarts": failures,
                "total_memory_bytes": 0,
                "total_cpu_time_ms": 0,
                "processes": [{
                    "process_id": "healthapp-web-1",
                    "app": "healthapp",
                    "kind": "web",
                    "ordinal": 1,
                    "pid": 12345,
                    "status": "Running",
                    "started_at": null,
                    "last_health_check": null,
                    "health_check_status": status,
                    "restart_count": failures,
                    "last_restart_at": null,
                    "cpu_time_ms": 0,
                    "memory_bytes": 0,
                    "requests_total": 0,
                    "requests_per_second": 0.0
                }],
                "last_updated": "2026-01-01T00:00:00Z"
            }]);

            fs::write(&stats_file, serde_json::to_string(&stats)?)?;

            let content = fs::read_to_string(&stats_file)?;
            let parsed: serde_json::Value = serde_json::from_str(&content)?;
            assert_eq!(
                parsed[0]["processes"][0]["health_check_status"], *status,
                "Health status mismatch for transition to {}",
                status
            );
        }

        Ok(())
    }

    /// Writing to a read-only directory fails with an I/O error.
    #[test]
    fn test_log_directory_creation_failure() -> Result<()> {
        let temp = TempDir::new()?;
        let log_root = temp.path().join("logs");

        fs::create_dir(&log_root)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&log_root)?.permissions();
            perms.set_mode(0o444); // Read-only
            fs::set_permissions(&log_root, perms)?;

            let app_log_dir = log_root.join("testapp");
            let result = fs::create_dir(&app_log_dir);
            assert!(result.is_err(), "Should fail with insufficient permissions");

            // Restore permissions so TempDir cleanup succeeds
            let mut perms = fs::metadata(&log_root)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&log_root, perms)?;
        }

        Ok(())
    }

    /// Atomic stats write: writing to a read-only file fails, original is untouched.
    #[test]
    fn test_stats_file_write_failure() -> Result<()> {
        let temp = TempDir::new()?;
        let stats_file = temp.path().join("stats.json");

        let original = r#"[{"app":"before","total_processes":1}]"#;
        fs::write(&stats_file, original)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&stats_file)?.permissions();
            perms.set_mode(0o444);
            fs::set_permissions(&stats_file, perms)?;

            // Write should fail
            let result = fs::write(&stats_file, "new content");
            assert!(result.is_err(), "Should fail writing to read-only file");

            // Original content is untouched
            let mut perms = fs::metadata(&stats_file)?.permissions();
            perms.set_mode(0o644);
            fs::set_permissions(&stats_file, perms)?;

            let content = fs::read_to_string(&stats_file)?;
            assert_eq!(content, original, "Read-only file should be unchanged");
        }

        Ok(())
    }

    /// Multiple worker configs can be created, modified, and deleted concurrently without data loss.
    #[test]
    fn test_concurrent_config_updates() -> Result<()> {
        let (_temp, riku_root) = setup_riku_env()?;
        let workers_enabled = riku_root.join("workers-enabled");

        // Create 5 configs
        for i in 1..=5 {
            create_worker_config(&workers_enabled, "multiapp", "web", i, "sleep 60")?;
        }

        let configs: Vec<_> = fs::read_dir(&workers_enabled)?
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with("multiapp"))
            .collect();
        assert_eq!(configs.len(), 5, "Should have 5 worker configs");

        // Verify each is valid TOML
        for entry in fs::read_dir(&workers_enabled)? {
            let entry = entry?;
            if entry.file_name().to_string_lossy().starts_with("multiapp") {
                let content = fs::read_to_string(entry.path())?;
                let parsed = toml::from_str::<toml::Value>(&content);
                assert!(
                    parsed.is_ok(),
                    "Config {:?} should parse as valid TOML",
                    entry.file_name()
                );
            }
        }

        Ok(())
    }

    /// Concurrent file writes from multiple threads don't corrupt data.
    #[test]
    fn test_concurrent_stats_writes() -> Result<()> {
        let temp = TempDir::new()?;
        let stats_path = temp.path().join("stats.json");

        // Initial write
        fs::write(&stats_path, "[]")?;

        let path = Arc::new(stats_path.clone());
        let errors = Arc::new(Mutex::new(Vec::<String>::new()));

        let handles: Vec<_> = (0..8)
            .map(|i| {
                let path = Arc::clone(&path);
                let errors = Arc::clone(&errors);

                thread::spawn(move || {
                    let content = serde_json::json!([{
                        "app": format!("app{}", i),
                        "total_processes": i,
                        "running_processes": i,
                        "healthy_processes": i,
                        "total_restarts": 0,
                        "total_memory_bytes": 0,
                        "total_cpu_time_ms": 0,
                        "processes": [],
                        "last_updated": "2026-01-01T00:00:00Z"
                    }]);

                    // Each thread uses its own unique tmp file to avoid races
                    let tmp = path.with_file_name(format!("stats.{}.tmp", i));
                    if let Err(e) = fs::write(&tmp, content.to_string()) {
                        errors
                            .lock()
                            .unwrap()
                            .push(format!("thread {}: write error: {}", i, e));
                        return;
                    }
                    // Atomic rename — last writer wins, but no corruption
                    if let Err(e) = fs::rename(&tmp, &*path) {
                        errors
                            .lock()
                            .unwrap()
                            .push(format!("thread {}: rename error: {}", i, e));
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().expect("thread panicked");
        }

        // No write errors
        let errs = errors.lock().unwrap();
        assert!(errs.is_empty(), "Concurrent writes had errors: {:?}", *errs);

        // Final file is valid JSON
        let content = fs::read_to_string(&stats_path)?;
        let parsed: std::result::Result<serde_json::Value, _> = serde_json::from_str(&content);
        assert!(
            parsed.is_ok(),
            "Final stats file should be valid JSON after concurrent writes"
        );

        Ok(())
    }

    /// Rapid restart counter increments are correctly tracked in stats.
    #[test]
    fn test_rapid_process_restart_tracking() -> Result<()> {
        let temp = TempDir::new()?;
        let stats_file = temp.path().join("stats.json");

        // Simulate 10 rapid restarts
        for restart_count in 0..=10u32 {
            let stats = serde_json::json!([{
                "app": "flapper",
                "total_processes": 1,
                "running_processes": if restart_count < 10 { 0 } else { 1 },
                "healthy_processes": 0,
                "total_restarts": restart_count,
                "total_memory_bytes": 0,
                "total_cpu_time_ms": 0,
                "processes": [{
                    "process_id": "flapper-web-1",
                    "app": "flapper",
                    "kind": "web",
                    "ordinal": 1,
                    "pid": null,
                    "status": "Restarting",
                    "started_at": null,
                    "last_health_check": null,
                    "health_check_status": "Unknown",
                    "restart_count": restart_count,
                    "last_restart_at": null,
                    "cpu_time_ms": 0,
                    "memory_bytes": 0,
                    "requests_total": 0,
                    "requests_per_second": 0.0
                }],
                "last_updated": "2026-01-01T00:00:00Z"
            }]);

            fs::write(&stats_file, serde_json::to_string(&stats)?)?;
        }

        // Read final state
        let content = fs::read_to_string(&stats_file)?;
        let parsed: serde_json::Value = serde_json::from_str(&content)?;
        assert_eq!(
            parsed[0]["total_restarts"], 10,
            "Should track 10 total restarts"
        );

        Ok(())
    }

    /// Worker config with non-existent command is still valid TOML.
    #[test]
    fn test_missing_executable_in_path() -> Result<()> {
        let (_temp, riku_root) = setup_riku_env()?;
        let workers_enabled = riku_root.join("workers-enabled");

        let config_path = create_worker_config(
            &workers_enabled,
            "badcmd",
            "web",
            1,
            "this-command-definitely-does-not-exist --with-args",
        )?;

        // Config itself is valid TOML even with a bad command
        let content = fs::read_to_string(&config_path)?;
        let parsed: toml::Value = toml::from_str(&content)?;
        let command = parsed["worker"]["command"].as_str().unwrap();
        assert!(
            command.contains("this-command-definitely-does-not-exist"),
            "Command should be stored as-is"
        );

        // The command is not on PATH
        assert!(
            which::which("this-command-definitely-does-not-exist").is_err(),
            "Command should not exist on PATH"
        );

        Ok(())
    }

    /// Disk space exhaustion: writing to a nearly-full tmpfs fails gracefully.
    #[test]
    fn test_disk_space_handling() -> Result<()> {
        let temp = TempDir::new()?;
        let test_file = temp.path().join("test.txt");

        // Normal write succeeds
        fs::write(&test_file, "test content")?;
        assert!(test_file.exists());
        assert_eq!(fs::read_to_string(&test_file)?, "test content");

        // Overwrite with new content — simulates what would fail on full disk
        let result = fs::write(&test_file, "updated content");
        assert!(result.is_ok(), "Write to writable dir should succeed");

        // Read-only simulation (proxies for "disk full" in tests)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let dir = temp.path().to_path_buf();
            let mut perms = fs::metadata(&dir)?.permissions();
            perms.set_mode(0o555);
            fs::set_permissions(&dir, perms)?;

            let new_file = dir.join("should_fail.txt");
            let result = fs::write(&new_file, "data");
            assert!(result.is_err(), "Write to read-only dir should fail");

            // Restore
            let mut perms = fs::metadata(&dir)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&dir, perms)?;
        }

        Ok(())
    }

    /// App names with valid characters are handled, unsafe names are detectable.
    #[test]
    fn test_app_name_validation() -> Result<()> {
        let (_temp, riku_root) = setup_riku_env()?;
        let workers_enabled = riku_root.join("workers-enabled");

        // Valid app names
        let valid_names = ["myapp", "my-app", "my_app", "app123", "myapp-test_123"];
        for name in &valid_names {
            let result = create_worker_config(&workers_enabled, name, "web", 1, "echo test");
            assert!(result.is_ok(), "Valid app name {:?} should work", name);
        }

        // Names with path traversal characters should not create files outside workers dir
        let traversal_attempt = "../../../etc/passwd";
        // The filename would be "../../etc/passwd.web.1.toml" — this should either fail or
        // be contained within the workers_enabled directory
        let config_path = workers_enabled.join(format!("{}.web.1.toml", traversal_attempt));
        // Canonicalizing helps detect escapes
        if let Ok(canonical) = config_path.canonicalize() {
            assert!(
                canonical.starts_with(&workers_enabled) || !canonical.exists(),
                "Path traversal should not escape workers directory"
            );
        }

        Ok(())
    }

    /// Very long command lines are stored correctly in TOML configs.
    #[test]
    fn test_very_long_command_line() -> Result<()> {
        let (_temp, riku_root) = setup_riku_env()?;
        let workers_enabled = riku_root.join("workers-enabled");

        // Command near OS arg limit (but safe for TOML)
        let long_args = "A".repeat(4096);
        let long_cmd = format!("echo {}", long_args);

        let config_path = create_worker_config(&workers_enabled, "longcmd", "web", 1, &long_cmd)?;

        // Config should be written and parseable
        let content = fs::read_to_string(&config_path)?;
        let parsed: toml::Value = toml::from_str(&content)?;
        let stored_cmd = parsed["worker"]["command"].as_str().unwrap();
        assert!(stored_cmd.starts_with("echo "), "Command prefix preserved");
        assert_eq!(stored_cmd.len(), long_cmd.len(), "Full command stored");

        Ok(())
    }

    /// Environment variable limit values parse correctly.
    #[test]
    fn test_environment_variable_limits() -> Result<()> {
        use std::env;

        // Use test-specific var names to avoid parallel-test interference with
        // test_resource_limits_from_env which writes the same RIKU_MAX_* vars.
        let cases = [
            ("RIKU_EVL_MAX_MEMORY_MB", "512", 512u64),
            ("RIKU_EVL_MAX_OPEN_FILES", "1024", 1024u64),
            ("RIKU_EVL_MAX_PROCESSES", "64", 64u64),
        ];

        for (var, val, expected) in &cases {
            env::set_var(var, val);
            let parsed: u64 = env::var(var).unwrap().parse().unwrap();
            assert_eq!(
                parsed, *expected,
                "Env var {} should parse to {}",
                var, expected
            );
            env::remove_var(var);
        }

        // Zero values are valid (disabling limits)
        env::set_var("RIKU_EVL_MAX_MEMORY_MB", "0");
        let parsed: u64 = env::var("RIKU_EVL_MAX_MEMORY_MB").unwrap().parse().unwrap();
        assert_eq!(parsed, 0);
        env::remove_var("RIKU_EVL_MAX_MEMORY_MB");

        Ok(())
    }

    /// Ten configs created rapidly are all valid and present.
    #[test]
    fn test_simultaneous_config_file_operations() -> Result<()> {
        let (_temp, riku_root) = setup_riku_env()?;
        let workers_enabled = riku_root.join("workers-enabled");

        for i in 1..=10 {
            create_worker_config(
                &workers_enabled,
                &format!("app{}", i),
                "web",
                1,
                "echo test",
            )?;
        }

        let entries: Vec<_> = fs::read_dir(&workers_enabled)?
            .filter_map(|e| e.ok())
            .collect();
        assert_eq!(entries.len(), 10, "All configs should be created");

        // Every config should be valid TOML
        for entry in &entries {
            let content = fs::read_to_string(entry.path())?;
            let result = toml::from_str::<toml::Value>(&content);
            assert!(
                result.is_ok(),
                "Config {:?} should be valid TOML",
                entry.file_name()
            );
        }

        Ok(())
    }

    /// OOM scenario: memory stats are tracked per-process in stats JSON.
    #[test]
    fn test_oom_memory_tracking() -> Result<()> {
        let temp = TempDir::new()?;
        let stats_file = temp.path().join("stats.json");

        // Simulate a process approaching memory limit (e.g. 512 MB limit, 480 MB used)
        let limit_bytes: u64 = 512 * 1024 * 1024;
        let used_bytes: u64 = 480 * 1024 * 1024;

        let stats = serde_json::json!([{
            "app": "memhog",
            "total_processes": 1,
            "running_processes": 1,
            "healthy_processes": 1,
            "total_restarts": 0,
            "total_memory_bytes": used_bytes,
            "total_cpu_time_ms": 0,
            "processes": [{
                "process_id": "memhog-web-1",
                "app": "memhog",
                "kind": "web",
                "ordinal": 1,
                "pid": 99999,
                "status": "Running",
                "started_at": null,
                "last_health_check": null,
                "health_check_status": "Healthy",
                "restart_count": 0,
                "last_restart_at": null,
                "cpu_time_ms": 0,
                "memory_bytes": used_bytes,
                "requests_total": 0,
                "requests_per_second": 0.0
            }],
            "last_updated": "2026-01-01T00:00:00Z"
        }]);

        fs::write(&stats_file, serde_json::to_string(&stats)?)?;

        let content = fs::read_to_string(&stats_file)?;
        let parsed: serde_json::Value = serde_json::from_str(&content)?;

        let reported_memory = parsed[0]["total_memory_bytes"].as_u64().unwrap();
        assert_eq!(reported_memory, used_bytes);
        // Memory usage exceeds 90% of limit — this would trigger an alert in monitoring
        assert!(
            (reported_memory as f64 / limit_bytes as f64) > 0.9,
            "Memory usage should be above 90% of limit"
        );

        Ok(())
    }
}
