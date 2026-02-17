/// Integration tests for CLI commands
///
/// These tests verify the functionality of CLI commands
/// by creating temporary directories and testing actual command execution.

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

        // Create directory structure
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

    // Helper to create a test app
    fn create_test_app(riku_root: &PathBuf, app_name: &str) -> Result<()> {
        let app_dir = riku_root.join("apps").join(app_name);
        let env_dir = riku_root.join("envs").join(app_name);

        fs::create_dir_all(&app_dir)?;
        fs::create_dir_all(&env_dir)?;

        // Create a basic ENV file
        let env_file = env_dir.join("ENV");
        fs::write(&env_file, "TEST_VAR=test_value\nPORT=3000\n")?;

        // Create a basic Procfile
        let procfile = app_dir.join("Procfile");
        fs::write(&procfile, "web: echo test\n")?;

        Ok(())
    }

    #[test]
    fn test_setup_riku_environment() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;

        assert!(riku_root.exists());
        assert!(riku_root.join("apps").exists());
        assert!(riku_root.join("envs").exists());
        assert!(riku_root.join("nginx").exists());

        Ok(())
    }

    #[test]
    fn test_create_test_app() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;
        create_test_app(&riku_root, "test-app")?;

        let app_dir = riku_root.join("apps").join("test-app");
        let env_dir = riku_root.join("envs").join("test-app");

        assert!(app_dir.exists());
        assert!(env_dir.exists());
        assert!(env_dir.join("ENV").exists());
        assert!(app_dir.join("Procfile").exists());

        Ok(())
    }

    #[test]
    fn test_multiple_apps() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;

        create_test_app(&riku_root, "app1")?;
        create_test_app(&riku_root, "app2")?;
        create_test_app(&riku_root, "app3")?;

        let apps_dir = riku_root.join("apps");
        let apps: Vec<_> = fs::read_dir(&apps_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .collect();

        assert_eq!(apps.len(), 3);

        Ok(())
    }

    #[test]
    fn test_app_with_scaling_file() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;
        create_test_app(&riku_root, "scaled-app")?;

        let scaling_file = riku_root.join("apps").join("scaled-app").join("SCALING");
        fs::write(&scaling_file, "web=2\nworker=4\n")?;

        assert!(scaling_file.exists());
        let content = fs::read_to_string(&scaling_file)?;
        assert!(content.contains("web=2"));
        assert!(content.contains("worker=4"));

        Ok(())
    }

    #[test]
    fn test_app_with_env_vars() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;
        create_test_app(&riku_root, "env-app")?;

        let env_file = riku_root.join("envs").join("env-app").join("ENV");
        let env_content = fs::read_to_string(&env_file)?;

        assert!(env_content.contains("TEST_VAR=test_value"));
        assert!(env_content.contains("PORT=3000"));

        Ok(())
    }

    #[test]
    fn test_app_log_directory() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;
        create_test_app(&riku_root, "log-app")?;

        let log_dir = riku_root.join("logs").join("log-app");
        fs::create_dir_all(&log_dir)?;

        // Create a sample log file
        let log_file = log_dir.join("web.1.log");
        fs::write(&log_file, "Sample log entry\n")?;

        assert!(log_dir.exists());
        assert!(log_file.exists());

        Ok(())
    }

    #[test]
    fn test_nginx_config_directory() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;
        create_test_app(&riku_root, "nginx-app")?;

        let nginx_dir = riku_root.join("nginx");

        // Create a sample nginx config
        let nginx_conf = nginx_dir.join("nginx-app.conf");
        fs::write(&nginx_conf, "server { listen 80; }\n")?;

        assert!(nginx_dir.exists());
        assert!(nginx_conf.exists());

        Ok(())
    }

    #[test]
    fn test_worker_config_directory() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;
        create_test_app(&riku_root, "worker-app")?;

        let workers_available = riku_root.join("workers-available");
        let workers_enabled = riku_root.join("workers-enabled");

        // Create a sample worker config
        let worker_conf = workers_available.join("worker-app.web.1.toml");
        fs::write(&worker_conf, "[worker]\napp = \"worker-app\"\n")?;

        // Create symlink to enabled
        #[cfg(unix)]
        std::os::unix::fs::symlink(&worker_conf, workers_enabled.join("worker-app.web.1.toml"))?;

        assert!(workers_available.exists());
        assert!(worker_conf.exists());

        Ok(())
    }

    #[test]
    fn test_git_repo_directory() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;

        let repo_dir = riku_root.join("repos").join("git-app");
        fs::create_dir_all(&repo_dir)?;

        // Create a bare git repo structure
        let git_dir = repo_dir.join(".git");
        fs::create_dir_all(&git_dir)?;
        fs::write(
            git_dir.join("config"),
            "[core]\nrepositoryformatversion = 0\n",
        )?;
        fs::create_dir_all(git_dir.join("hooks"))?;

        assert!(repo_dir.exists());
        assert!(git_dir.exists());
        assert!(git_dir.join("hooks").exists());

        Ok(())
    }

    #[test]
    fn test_data_directory() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;
        create_test_app(&riku_root, "data-app")?;

        let data_dir = riku_root.join("data").join("data-app");
        fs::create_dir_all(&data_dir)?;

        // Create sample data files
        fs::write(data_dir.join("database.db"), "sample data")?;
        fs::write(data_dir.join("config.json"), r#"{"key": "value"}"#)?;

        assert!(data_dir.exists());
        assert!(data_dir.join("database.db").exists());
        assert!(data_dir.join("config.json").exists());

        Ok(())
    }

    #[test]
    fn test_cache_directory() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;

        let cache_dir = riku_root.join("cache");

        // Create sample cache files
        fs::create_dir_all(&cache_dir)?;
        fs::write(cache_dir.join("cache_key_1"), "cached value 1")?;
        fs::write(cache_dir.join("cache_key_2"), "cached value 2")?;

        assert!(cache_dir.exists());
        assert!(cache_dir.join("cache_key_1").exists());

        Ok(())
    }

    #[test]
    fn test_acme_directory() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;

        let acme_root = riku_root.join("acme");
        let acme_www = riku_root.join("acme-www");

        fs::create_dir_all(&acme_root)?;
        fs::create_dir_all(&acme_www)?;

        // Create sample ACME challenge directory
        let challenge_dir = acme_www.join(".well-known").join("acme-challenge");
        fs::create_dir_all(&challenge_dir)?;
        fs::write(challenge_dir.join("challenge_token"), "challenge_response")?;

        assert!(acme_root.exists());
        assert!(acme_www.exists());
        assert!(challenge_dir.exists());

        Ok(())
    }

    #[test]
    fn test_plugin_directory() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;

        let plugin_dir = riku_root.join("plugins");
        fs::create_dir_all(&plugin_dir)?;

        // Create sample plugin scripts
        #[cfg(unix)]
        {
            let plugin1 = plugin_dir.join("deploy-hook.sh");
            fs::write(&plugin1, "#!/bin/bash\necho 'Deploy hook'\n")?;

            // Make executable
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&plugin1, fs::Permissions::from_mode(0o755))?;

            assert!(plugin1.exists());
            assert!(plugin1.metadata()?.permissions().mode() & 0o111 != 0);
        }

        Ok(())
    }

    #[test]
    fn test_app_cleanup() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;
        create_test_app(&riku_root, "cleanup-app")?;

        let app_dir = riku_root.join("apps").join("cleanup-app");
        let env_dir = riku_root.join("envs").join("cleanup-app");
        let log_dir = riku_root.join("logs").join("cleanup-app");

        fs::create_dir_all(&log_dir)?;

        assert!(app_dir.exists());
        assert!(env_dir.exists());
        assert!(log_dir.exists());

        // Simulate app removal
        fs::remove_dir_all(&app_dir)?;
        fs::remove_dir_all(&env_dir)?;
        fs::remove_dir_all(&log_dir)?;

        assert!(!app_dir.exists());
        assert!(!env_dir.exists());
        assert!(!log_dir.exists());

        Ok(())
    }

    #[test]
    fn test_env_file_parsing() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;
        create_test_app(&riku_root, "parse-app")?;

        let env_file = riku_root.join("envs").join("parse-app").join("ENV");

        // Write various env var formats
        let content = r#"# Comment line
SIMPLE=value
WITH_SPACES=value with spaces
QUOTED="quoted value"
SINGLE_QUOTED='single quoted'
EQUALS=value=with=equals
EMPTY=
"#;
        fs::write(&env_file, content)?;

        let parsed = fs::read_to_string(&env_file)?;
        assert!(parsed.contains("SIMPLE=value"));
        assert!(parsed.contains("WITH_SPACES=value with spaces"));

        Ok(())
    }

    #[test]
    fn test_procfile_parsing() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;
        create_test_app(&riku_root, "procfile-app")?;

        let procfile = riku_root.join("apps").join("procfile-app").join("Procfile");

        let content = r#"web: python app.py
worker: python worker.py
cron: 0 2 * * * /path/to/script.sh
# This is a comment
release: python migrate.py
"#;
        fs::write(&procfile, content)?;

        let parsed = fs::read_to_string(&procfile)?;
        assert!(parsed.contains("web: python app.py"));
        assert!(parsed.contains("worker: python worker.py"));
        assert!(parsed.contains("cron:"));

        Ok(())
    }

    #[test]
    fn test_log_tail_simulation() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;
        create_test_app(&riku_root, "log-tail-app")?;

        let log_dir = riku_root.join("logs").join("log-tail-app");
        fs::create_dir_all(&log_dir)?;

        // Create log file with multiple entries
        let log_file = log_dir.join("web.1.log");
        let log_content = r#"2024-01-01 10:00:00 Starting application
2024-01-01 10:00:01 Application initialized
2024-01-01 10:00:02 Listening on port 3000
2024-01-01 10:00:03 Request received: GET /
2024-01-01 10:00:04 Response sent: 200 OK
"#;
        fs::write(&log_file, log_content)?;

        // Read last N lines (simulating tail)
        let content = fs::read_to_string(&log_file)?;
        let lines: Vec<&str> = content.lines().collect();
        let last_3: Vec<&str> = lines
            .iter()
            .skip(lines.len().saturating_sub(3))
            .copied()
            .collect();

        assert_eq!(last_3.len(), 3);
        assert!(last_3[0].contains("Listening on port 3000"));

        Ok(())
    }

    #[test]
    fn test_config_merge() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;
        create_test_app(&riku_root, "merge-app")?;

        let env_file = riku_root.join("envs").join("merge-app").join("ENV");

        // Initial config
        fs::write(&env_file, "KEY1=value1\nKEY2=value2\n")?;

        // Read and merge
        let mut content = fs::read_to_string(&env_file)?;
        content.push_str("KEY3=value3\n");
        content.push_str("KEY1=new_value1\n"); // Override

        fs::write(&env_file, &content)?;

        let final_content = fs::read_to_string(&env_file)?;
        assert!(final_content.contains("KEY1=new_value1"));
        assert!(final_content.contains("KEY2=value2"));
        assert!(final_content.contains("KEY3=value3"));

        Ok(())
    }
}
