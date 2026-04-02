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
            working_dir: "/home/deploy/.riku/apps/myapp".to_string(),
            log_file: "/home/deploy/.riku/logs/myapp/web.1.log".to_string(),
            uid: Some("deploy".to_string()),
            gid: Some("deploy".to_string()),
            timeout: default_timeout(),
            grace_period: default_grace_period(),
            max_restarts: default_max_restarts(),
            health_check: None,
        },
    };

    let toml_str = toml::to_string(&config).unwrap();
    tracing::debug!("Serialized config:\n{}", toml_str);

    let parsed: WorkerConfig = toml::from_str(&toml_str).unwrap();
    assert_eq!(parsed.worker.app, "myapp");
    assert_eq!(parsed.worker.kind, "web");
    assert_eq!(parsed.worker.command, "python app.py");
    assert_eq!(parsed.worker.ordinal, 1);
    assert_eq!(parsed.env.get("PORT").unwrap(), "8080");
    assert_eq!(parsed.options.working_dir, "/home/deploy/.riku/apps/myapp");
    assert_eq!(parsed.options.uid, Some("deploy".to_string()));
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
    assert_eq!(
        config.env.get("BIND_ADDRESS"),
        Some(&"127.0.0.1".to_string())
    );
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
    assert_eq!(
        config.options.grace_period,
        crate::config::RIKU_WORKER_GRACE_PERIOD
    );
    assert_eq!(
        config.options.max_restarts,
        crate::config::RIKU_MAX_RESTARTS
    );
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
    assert_eq!(
        config.options.max_restarts,
        crate::config::RIKU_MAX_RESTARTS
    );
}

#[test]
fn test_create_worker_config_with_health_check() {
    let mut env = HashMap::new();
    env.insert(
        "RIKU_HEALTH_CHECK_URL".to_string(),
        "/api/health".to_string(),
    );
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
    env.insert(
        "RIKU_WORKER_PROCESSES".to_string(),
        "web=4,worker=2".to_string(),
    );

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
