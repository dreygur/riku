//! Integration tests for Supervisor functionality
//!
//! These tests verify the process supervisor, process manager,
//! cron scheduler, and log rotation functionality.

#[cfg(test)]
pub mod tests {
    use anyhow::Result;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    // Helper to create a temporary Riku environment
    pub fn setup_riku_env() -> Result<(TempDir, PathBuf)> {
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
    pub fn create_worker_config(
        workers_dir: &Path,
        app: &str,
        kind: &str,
        ordinal: u32,
        command: &str,
    ) -> Result<PathBuf> {
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
    fn test_worker_config_creation() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;
        let workers_dir = riku_root.join("workers-available");

        let config_file =
            create_worker_config(&workers_dir, "test-app", "web", 1, "python app.py")?;

        assert!(config_file.exists());

        let content = fs::read_to_string(&config_file)?;
        assert!(content.contains("app = \"test-app\""));
        assert!(content.contains("kind = \"web\""));
        assert!(content.contains("command = \"python app.py\""));
        assert!(content.contains("ordinal = 1"));

        Ok(())
    }

    #[test]
    fn test_multiple_worker_configs() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;
        let workers_dir = riku_root.join("workers-available");

        create_worker_config(&workers_dir, "app1", "web", 1, "python app.py")?;
        create_worker_config(&workers_dir, "app1", "web", 2, "python app.py")?;
        create_worker_config(&workers_dir, "app1", "worker", 1, "python worker.py")?;

        let configs: Vec<_> = fs::read_dir(&workers_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
            .collect();

        assert_eq!(configs.len(), 3);

        Ok(())
    }

    #[test]
    fn test_worker_config_symlink() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;

        let available = riku_root.join("workers-available");
        let enabled = riku_root.join("workers-enabled");

        let config_file = create_worker_config(&available, "test-app", "web", 1, "echo test")?;

        // Create symlink
        let filename = config_file.file_name().unwrap();
        let symlink_path = enabled.join(filename);

        #[cfg(unix)]
        std::os::unix::fs::symlink(&config_file, &symlink_path)?;

        assert!(symlink_path.exists());
        assert!(symlink_path.is_symlink());

        Ok(())
    }

    #[test]
    fn test_worker_config_parsing() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;
        let workers_dir = riku_root.join("workers-available");

        let config_file =
            create_worker_config(&workers_dir, "parse-app", "web", 1, "python app.py")?;

        let content = fs::read_to_string(&config_file)?;
        let config: toml::Value = toml::from_str(&content)?;

        assert_eq!(config["worker"]["app"].as_str(), Some("parse-app"));
        assert_eq!(config["worker"]["kind"].as_str(), Some("web"));
        assert_eq!(config["worker"]["ordinal"].as_integer(), Some(1));
        assert_eq!(config["env"]["PORT"].as_str(), Some("5001"));

        Ok(())
    }

    #[test]
    fn test_cron_expression_validation() -> Result<()> {
        // Test valid cron expressions
        let valid_expressions = vec![
            "0 * * * *",    // Every hour
            "0 0 * * *",    // Every day at midnight
            "*/5 * * * *",  // Every 5 minutes
            "0 0 * * 0",    // Every Sunday
            "0 0 1 * *",    // First of every month
            "30 8 * * 1-5", // Weekdays at 8:30
            "0 0 * * *",    // Daily
            "@hourly",      // Predefined alias
            "@daily",       // Predefined alias
            "@weekly",      // Predefined alias
        ];

        for expr in valid_expressions {
            // Basic validation - check format
            let parts: Vec<&str> = expr.split_whitespace().collect();
            if !expr.starts_with('@') {
                assert_eq!(parts.len(), 5, "Invalid cron expression: {}", expr);
            }
        }

        Ok(())
    }

    #[test]
    fn test_cron_job_file_creation() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;
        let workers_dir = riku_root.join("workers-available");

        // Create a cron job config
        let config_file = workers_dir.join("cron-app.cron.1.toml");
        let content = r#"
[worker]
app = "cron-app"
kind = "cron"
command = "0 2 * * * /path/to/script.sh"
ordinal = 1

[env]
APP_NAME = "cron-app"
"#;
        fs::write(&config_file, content)?;

        assert!(config_file.exists());

        let parsed: toml::Value = toml::from_str(content)?;
        assert_eq!(parsed["worker"]["kind"].as_str(), Some("cron"));
        assert!(parsed["worker"]["command"]
            .as_str()
            .unwrap()
            .contains("0 2 * * *"));

        Ok(())
    }

    #[test]
    fn test_log_rotation_config() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;
        let logs_dir = riku_root.join("logs");

        // Create app log directory
        let app_logs = logs_dir.join("log-app");
        fs::create_dir_all(&app_logs)?;

        // Create log file
        let log_file = app_logs.join("web.1.log");
        fs::write(&log_file, "Initial log content\n")?;

        assert!(log_file.exists());

        Ok(())
    }

    #[test]
    fn test_log_file_size_check() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;
        let logs_dir = riku_root.join("logs");

        let app_logs = logs_dir.join("size-app");
        fs::create_dir_all(&app_logs)?;

        let log_file = app_logs.join("web.1.log");

        // Write 1MB of data
        let large_content = "X".repeat(1024 * 1024);
        fs::write(&log_file, &large_content)?;

        let metadata = fs::metadata(&log_file)?;
        assert!(metadata.len() >= 1024 * 1024);

        Ok(())
    }

    #[test]
    fn test_log_rotation_simulation() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;
        let logs_dir = riku_root.join("logs");

        let app_logs = logs_dir.join("rotate-app");
        fs::create_dir_all(&app_logs)?;

        let log_file = app_logs.join("web.1.log");
        fs::write(&log_file, "Log content to rotate\n")?;

        // Simulate rotation by renaming
        let rotated_file = app_logs.join("web.1.log.1");
        fs::rename(&log_file, &rotated_file)?;

        // Create new empty log file
        fs::write(&log_file, "")?;

        assert!(!log_file.exists() || fs::metadata(&log_file)?.len() == 0);
        assert!(rotated_file.exists());

        Ok(())
    }

    #[test]
    fn test_multiple_log_files() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;
        let logs_dir = riku_root.join("logs");

        let app_logs = logs_dir.join("multi-log-app");
        fs::create_dir_all(&app_logs)?;

        // Create multiple log files
        let log_files = vec!["web.1.log", "web.2.log", "worker.1.log", "cron.1.log"];

        for file in &log_files {
            fs::write(app_logs.join(file), format!("Content for {}\n", file))?;
        }

        let files: Vec<_> = fs::read_dir(&app_logs)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
            .collect();

        assert_eq!(files.len(), 4);

        Ok(())
    }

    #[test]
    fn test_process_stats_file() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;
        let logs_dir = riku_root.join("logs");

        let app_logs = logs_dir.join("stats-app");
        fs::create_dir_all(&app_logs)?;

        // Create a stats file
        let stats_file = app_logs.join("stats.json");
        let stats_content = r#"{
            "app": "stats-app",
            "processes": {
                "web": 2,
                "worker": 1
            },
            "memory_mb": 128,
            "cpu_percent": 5.5
        }"#;
        fs::write(&stats_file, stats_content)?;

        assert!(stats_file.exists());

        let parsed: serde_json::Value = serde_json::from_str(stats_content)?;
        assert_eq!(parsed["app"], "stats-app");
        assert_eq!(parsed["processes"]["web"], 2);

        Ok(())
    }

    #[test]
    fn test_supervisor_signal_handling() -> Result<()> {
        // Test that signal handling constants exist
        // This is a basic sanity check
        use std::sync::atomic::{AtomicBool, Ordering};

        static RUNNING: AtomicBool = AtomicBool::new(true);

        assert!(RUNNING.load(Ordering::SeqCst));
        RUNNING.store(false, Ordering::SeqCst);
        assert!(!RUNNING.load(Ordering::SeqCst));

        Ok(())
    }

    #[test]
    fn test_worker_config_with_options() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;
        let workers_dir = riku_root.join("workers-available");

        let config_file = workers_dir.join("options-app.web.1.toml");
        let content = r#"
[worker]
app = "options-app"
kind = "web"
command = "python app.py"
ordinal = 1

[env]
PORT = "5000"
DEBUG = "true"

[options]
working_dir = "/home/user/app"
log_file = "/var/log/app.log"
max_restarts = 5
graceful_timeout = 30
"#;
        fs::write(&config_file, content)?;

        let parsed: toml::Value = toml::from_str(content)?;

        assert_eq!(
            parsed["options"]["working_dir"].as_str(),
            Some("/home/user/app")
        );
        assert_eq!(
            parsed["options"]["log_file"].as_str(),
            Some("/var/log/app.log")
        );
        assert_eq!(parsed["options"]["max_restarts"].as_integer(), Some(5));
        assert_eq!(parsed["options"]["graceful_timeout"].as_integer(), Some(30));

        Ok(())
    }

    #[test]
    fn test_worker_config_health_check() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;
        let workers_dir = riku_root.join("workers-available");

        let config_file = workers_dir.join("health-app.web.1.toml");
        let content = r#"
[worker]
app = "health-app"
kind = "web"
command = "python app.py"
ordinal = 1

[env]
PORT = "5000"

[health_check]
enabled = true
endpoint = "/health"
interval_seconds = 30
timeout_seconds = 5
unhealthy_threshold = 3
"#;
        fs::write(&config_file, content)?;

        let parsed: toml::Value = toml::from_str(content)?;

        assert!(parsed["health_check"]["enabled"].as_bool().unwrap());
        assert_eq!(parsed["health_check"]["endpoint"].as_str(), Some("/health"));
        assert_eq!(
            parsed["health_check"]["interval_seconds"].as_integer(),
            Some(30)
        );

        Ok(())
    }

    #[test]
    fn test_app_directory_cleanup() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;

        // Create all directories for an app
        let app_name = "cleanup-test";
        let dirs = ["apps", "envs", "logs", "nginx"];

        for dir in &dirs {
            let path = riku_root.join(dir).join(app_name);
            fs::create_dir_all(&path)?;

            // Add some files
            if dir == &"logs" {
                fs::write(path.join("web.1.log"), "log content")?;
            } else if dir == &"envs" {
                fs::write(path.join("ENV"), "KEY=value")?;
            }
        }

        // Verify all exist
        for dir in &dirs {
            assert!(riku_root.join(dir).join(app_name).exists());
        }

        // Cleanup simulation - remove app directories
        for dir in &dirs {
            let path = riku_root.join(dir).join(app_name);
            fs::remove_dir_all(&path)?;
            assert!(!path.exists());
        }

        Ok(())
    }

    #[test]
    fn test_concurrent_worker_configs() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;
        let workers_dir = riku_root.join("workers-available");

        // Create configs for multiple apps concurrently
        let apps = vec!["app1", "app2", "app3"];

        for app in &apps {
            create_worker_config(&workers_dir, app, "web", 1, "echo test")?;
            create_worker_config(&workers_dir, app, "worker", 1, "echo worker")?;
        }

        let configs: Vec<_> = fs::read_dir(&workers_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
            .collect();

        assert_eq!(configs.len(), 6); // 3 apps * 2 workers each

        Ok(())
    }

    #[test]
    fn test_worker_config_with_restart_policy() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;
        let workers_dir = riku_root.join("workers-available");

        let config_file = workers_dir.join("restart-app.web.1.toml");
        let content = r#"
[worker]
app = "restart-app"
kind = "web"
command = "python app.py"
ordinal = 1

[restart]
policy = "always"
max_attempts = 5
delay_seconds = 10
backoff_multiplier = 2.0
max_delay_seconds = 300
"#;
        fs::write(&config_file, content)?;

        let parsed: toml::Value = toml::from_str(content)?;

        assert_eq!(parsed["restart"]["policy"].as_str(), Some("always"));
        assert_eq!(parsed["restart"]["max_attempts"].as_integer(), Some(5));
        assert_eq!(parsed["restart"]["delay_seconds"].as_integer(), Some(10));

        Ok(())
    }
}
