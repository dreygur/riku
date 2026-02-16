//! Worker configuration module for the supervisor.
//!
//! Defines the structure for TOML-based worker configurations.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The main worker configuration structure stored in TOML files.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WorkerConfig {
    pub worker: WorkerInfo,
    pub env: HashMap<String, String>,
    pub options: WorkerOptions,
}

/// Information about the worker process.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WorkerInfo {
    pub app: String,
    pub kind: String,
    pub command: String,
    pub ordinal: u32,
}

/// Options for the worker process.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WorkerOptions {
    pub working_dir: String,
    pub log_file: String,
    #[serde(default)]
    pub uid: Option<String>,
    #[serde(default)]
    pub gid: Option<String>,
    #[serde(default = "default_timeout")]
    pub timeout: u64,
    #[serde(default = "default_grace_period")]
    pub grace_period: u64,
    #[serde(default = "default_max_restarts")]
    pub max_restarts: u32,
    #[serde(default)]
    pub health_check: Option<HealthCheck>,
}

/// Health check configuration for a worker.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HealthCheck {
    #[serde(default = "default_health_check_url")]
    pub url: String,
    #[serde(default = "default_health_check_interval")]
    pub interval: u64,
    #[serde(default = "default_health_check_timeout")]
    pub timeout: u64,
    #[serde(default = "default_health_check_retries")]
    pub retries: u32,
}

fn default_health_check_url() -> String {
    "/health".to_string()
}

fn default_health_check_interval() -> u64 {
    30
}

fn default_health_check_timeout() -> u64 {
    5
}

fn default_health_check_retries() -> u32 {
    3
}

fn default_timeout() -> u64 {
    crate::config::RIKU_WORKER_TIMEOUT
}

fn default_grace_period() -> u64 {
    crate::config::RIKU_WORKER_GRACE_PERIOD
}

fn default_max_restarts() -> u32 {
    crate::config::RIKU_MAX_RESTARTS
}

impl Default for WorkerConfig {
    fn default() -> Self {
        WorkerConfig {
            worker: WorkerInfo {
                app: String::new(),
                kind: String::new(),
                command: String::new(),
                ordinal: 0,
            },
            env: HashMap::new(),
            options: WorkerOptions {
                working_dir: String::new(),
                log_file: String::new(),
                uid: None,
                gid: None,
                timeout: default_timeout(),
                grace_period: default_grace_period(),
                max_restarts: default_max_restarts(),
                health_check: None,
            },
        }
    }
}

/// Create a worker config from app name, kind, command, and environment.
/// Reads RIKU_* environment variables for worker management settings.
pub fn create_worker_config(
    app: &str,
    kind: &str,
    command: &str,
    ordinal: u32,
    mut env: HashMap<String, String>,
    working_dir: &str,
    log_file: &str,
) -> WorkerConfig {
    // Read RIKU_* settings from environment with defaults
    let timeout = env
        .get("RIKU_WORKER_TIMEOUT")
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or_else(default_timeout);

    let grace_period = env
        .get("RIKU_WORKER_GRACE_PERIOD")
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or_else(default_grace_period);

    let max_restarts = env
        .get("RIKU_MAX_RESTARTS")
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or_else(default_max_restarts);

    // Add BIND_ADDRESS to worker env if not already set
    if !env.contains_key("BIND_ADDRESS") {
        env.insert("BIND_ADDRESS".to_string(), "127.0.0.1".to_string());
    }

    // Read health check settings from environment
    let health_check = env.get("RIKU_HEALTH_CHECK_URL").map(|_| HealthCheck {
        url: env
            .get("RIKU_HEALTH_CHECK_URL")
            .cloned()
            .unwrap_or_else(default_health_check_url),
        interval: env
            .get("RIKU_HEALTH_CHECK_INTERVAL")
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or_else(default_health_check_interval),
        timeout: env
            .get("RIKU_HEALTH_CHECK_TIMEOUT")
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or_else(default_health_check_timeout),
        retries: env
            .get("RIKU_HEALTH_CHECK_RETRIES")
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or_else(default_health_check_retries),
    });

    WorkerConfig {
        worker: WorkerInfo {
            app: app.to_string(),
            kind: kind.to_string(),
            command: command.to_string(),
            ordinal,
        },
        env,
        options: WorkerOptions {
            working_dir: working_dir.to_string(),
            log_file: log_file.to_string(),
            uid: None,
            gid: None,
            timeout,
            grace_period,
            max_restarts,
            health_check,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_worker_config_serialization() {
        let mut env = HashMap::new();
        env.insert("PORT".to_string(), "8080".to_string());
        env.insert(
            "DATABASE_URL".to_string(),
            "sqlite:///db.sqlite3".to_string(),
        );

        let config = WorkerConfig {
            worker: WorkerInfo {
                app: "myapp".to_string(),
                kind: "web".to_string(),
                command: "python app.py".to_string(),
                ordinal: 1,
            },
            env,
            options: WorkerOptions {
                working_dir: "/home/piku/.piku/apps/myapp".to_string(),
                log_file: "/home/piku/.piku/logs/myapp/web.1.log".to_string(),
                uid: Some("piku".to_string()),
                gid: Some("piku".to_string()),
                timeout: default_timeout(),
                grace_period: default_grace_period(),
                max_restarts: default_max_restarts(),
                health_check: None,
            },
        };

        let toml_str = toml::to_string(&config).unwrap();
        println!("Serialized config:\n{}", toml_str);

        let parsed: WorkerConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.worker.app, "myapp");
        assert_eq!(parsed.worker.kind, "web");
        assert_eq!(parsed.worker.command, "python app.py");
        assert_eq!(parsed.worker.ordinal, 1);
        assert_eq!(parsed.env.get("PORT").unwrap(), "8080");
        assert_eq!(parsed.options.working_dir, "/home/piku/.piku/apps/myapp");
        assert_eq!(parsed.options.uid, Some("piku".to_string()));
    }

    #[test]
    fn test_create_worker_config_helper() {
        let mut env = HashMap::new();
        env.insert("PORT".to_string(), "5000".to_string());

        let config = create_worker_config(
            "testapp",
            "worker",
            "python worker.py",
            1,
            env,
            "/path/to/app",
            "/path/to/log",
        );

        assert_eq!(config.worker.app, "testapp");
        assert_eq!(config.worker.kind, "worker");
        assert_eq!(config.worker.command, "python worker.py");
        assert_eq!(config.worker.ordinal, 1);
        assert_eq!(config.options.working_dir, "/path/to/app");
        assert_eq!(config.options.log_file, "/path/to/log");
    }

    #[test]
    fn test_create_worker_config_with_riku_vars() {
        let mut env = HashMap::new();
        env.insert("PORT".to_string(), "5000".to_string());
        env.insert("RIKU_WORKER_TIMEOUT".to_string(), "3600".to_string());
        env.insert("RIKU_WORKER_GRACE_PERIOD".to_string(), "60".to_string());
        env.insert("RIKU_MAX_RESTARTS".to_string(), "10".to_string());

        let config = create_worker_config(
            "testapp",
            "web",
            "python app.py",
            1,
            env.clone(),
            "/path/to/app",
            "/path/to/log",
        );

        assert_eq!(config.options.timeout, 3600);
        assert_eq!(config.options.grace_period, 60);
        assert_eq!(config.options.max_restarts, 10);
        
        // BIND_ADDRESS should be added automatically
        assert_eq!(config.env.get("BIND_ADDRESS"), Some(&"127.0.0.1".to_string()));
    }

    #[test]
    fn test_create_worker_config_default_riku_vars() {
        let env = HashMap::new();

        let config = create_worker_config(
            "testapp",
            "web",
            "python app.py",
            1,
            env,
            "/path/to/app",
            "/path/to/log",
        );

        // Should use defaults from config constants
        assert_eq!(config.options.timeout, crate::config::RIKU_WORKER_TIMEOUT);
        assert_eq!(config.options.grace_period, crate::config::RIKU_WORKER_GRACE_PERIOD);
        assert_eq!(config.options.max_restarts, crate::config::RIKU_MAX_RESTARTS);
    }

    #[test]
    fn test_create_worker_config_invalid_riku_vars() {
        let mut env = HashMap::new();
        env.insert("RIKU_WORKER_TIMEOUT".to_string(), "invalid".to_string());
        env.insert("RIKU_MAX_RESTARTS".to_string(), "not-a-number".to_string());

        let config = create_worker_config(
            "testapp",
            "web",
            "python app.py",
            1,
            env,
            "/path/to/app",
            "/path/to/log",
        );

        // Should fall back to defaults when parsing fails
        assert_eq!(config.options.timeout, crate::config::RIKU_WORKER_TIMEOUT);
        assert_eq!(config.options.max_restarts, crate::config::RIKU_MAX_RESTARTS);
    }

    #[test]
    fn test_create_worker_config_with_health_check() {
        let mut env = HashMap::new();
        env.insert("RIKU_HEALTH_CHECK_URL".to_string(), "/api/health".to_string());
        env.insert("RIKU_HEALTH_CHECK_INTERVAL".to_string(), "60".to_string());
        env.insert("RIKU_HEALTH_CHECK_TIMEOUT".to_string(), "10".to_string());
        env.insert("RIKU_HEALTH_CHECK_RETRIES".to_string(), "5".to_string());

        let config = create_worker_config(
            "testapp",
            "web",
            "python app.py",
            1,
            env,
            "/path/to/app",
            "/path/to/log",
        );

        let health_check = config.options.health_check.unwrap();
        assert_eq!(health_check.url, "/api/health");
        assert_eq!(health_check.interval, 60);
        assert_eq!(health_check.timeout, 10);
        assert_eq!(health_check.retries, 5);
    }

    #[test]
    fn test_create_worker_config_health_check_defaults() {
        let env = HashMap::new();

        let config = create_worker_config(
            "testapp",
            "web",
            "python app.py",
            1,
            env,
            "/path/to/app",
            "/path/to/log",
        );

        // Health check should be None if not configured
        assert!(config.options.health_check.is_none());
    }

    #[test]
    fn test_create_worker_config_with_worker_processes() {
        let mut env = HashMap::new();
        env.insert("RIKU_WORKER_PROCESSES".to_string(), "web=4,worker=2".to_string());

        let config = create_worker_config(
            "testapp",
            "web",
            "python app.py",
            1,
            env,
            "/path/to/app",
            "/path/to/log",
        );

        // RIKU_WORKER_PROCESSES should be in env
        assert_eq!(
            config.env.get("RIKU_WORKER_PROCESSES"),
            Some(&"web=4,worker=2".to_string())
        );
    }
}
