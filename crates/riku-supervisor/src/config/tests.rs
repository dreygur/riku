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
            limits: None,
            sandbox: None,
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

    // RIKU_BIND_ADDRESS should be added automatically
    assert_eq!(
        config.env.get("RIKU_BIND_ADDRESS"),
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
    assert_eq!(config.options.timeout, riku_config::RIKU_WORKER_TIMEOUT);
    assert_eq!(
        config.options.grace_period,
        riku_config::RIKU_WORKER_GRACE_PERIOD
    );
    assert_eq!(config.options.max_restarts, riku_config::RIKU_MAX_RESTARTS);
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
    assert_eq!(config.options.timeout, riku_config::RIKU_WORKER_TIMEOUT);
    assert_eq!(config.options.max_restarts, riku_config::RIKU_MAX_RESTARTS);
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

fn config_from_env(pairs: &[(&str, &str)]) -> WorkerConfig {
    let env = pairs
        .iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect();
    create_worker_config("app", "web", "run", 1, env, "/srv/app", "/srv/log")
}

#[test]
fn memory_limit_without_isolation_root_gets_a_cgroup_and_no_sandbox() {
    let config = config_from_env(&[("RIKU_ISOLATION_MAX_MEMORY_MB", "256")]);

    let limits = config
        .options
        .limits
        .expect("a memory limit alone must still give the worker cgroup limits");
    assert_eq!(limits.max_memory_bytes, Some(256 * 1024 * 1024));
    assert!(
        config.options.sandbox.is_none(),
        "limits alone must not chroot or unshare namespaces"
    );
}

#[test]
fn memory_limit_too_large_for_bytes_is_ignored_like_a_non_number() {
    let too_large = (u64::MAX / (1024 * 1024) + 1).to_string();
    let config = config_from_env(&[
        ("RIKU_ISOLATION_MAX_MEMORY_MB", too_large.as_str()),
        ("RIKU_ISOLATION_MAX_PIDS", "64"),
    ]);

    let limits = config.options.limits.expect("the pids limit is set");
    assert_eq!(
        limits.max_memory_bytes, None,
        "{too_large} MB does not fit in u64 bytes and must not wrap to a small memory.max"
    );
}

#[test]
fn isolation_root_turns_on_the_sandbox_and_keeps_the_limits() {
    let config = config_from_env(&[
        ("RIKU_ISOLATION_ROOT", "/srv/jail"),
        ("RIKU_ISOLATION_MAX_PIDS", "64"),
    ]);

    let sandbox = config.options.sandbox.expect("RIKU_ISOLATION_ROOT is set");
    assert_eq!(sandbox.root_dir, "/srv/jail");
    let pids = config.options.limits.and_then(|limits| limits.max_pids);
    assert_eq!(pids, Some(64), "the sandbox must not drop the limits");
}

#[test]
fn sandbox_root_must_be_an_existing_absolute_directory() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("file");
    std::fs::write(&file, "").unwrap();
    let missing = tmp.path().join("missing");

    for root in [
        "srv/jail".to_string(),
        missing.display().to_string(),
        file.display().to_string(),
    ] {
        let sandbox = Sandbox {
            root_dir: root.clone(),
        };
        match sandbox.check_root() {
            Ok(()) => panic!("accepted {root} as a sandbox root"),
            Err(e) => assert!(e.to_string().contains(&root), "names the root: {e}"),
        }
    }

    let sandbox = Sandbox {
        root_dir: tmp.path().display().to_string(),
    };
    assert!(
        sandbox.check_root().is_ok(),
        "an existing directory is a valid root"
    );
}

/// A hand edited worker config skips the settings check, so the
/// supervisor applies the same shape rule to every root it is given.
#[test]
fn sandbox_root_must_have_the_settings_shape_whatever_its_source() {
    let tmp = tempfile::tempdir().unwrap();
    let spaced = tmp.path().join("a b");
    std::fs::create_dir(&spaced).unwrap();
    let dotted = tmp.path().join("x");
    std::fs::create_dir(&dotted).unwrap();

    for root in [
        spaced.display().to_string(),
        format!("{}/..", dotted.display()),
        "/".to_string(),
    ] {
        let sandbox = Sandbox {
            root_dir: root.clone(),
        };
        assert!(
            sandbox.check_root().is_err(),
            "accepted {root} as a sandbox root"
        );
    }
}

#[test]
fn cpu_period_alone_asks_for_no_cgroup() {
    let config = config_from_env(&[("RIKU_ISOLATION_CPU_PERIOD_US", "50000")]);

    assert!(
        config.options.limits.is_none(),
        "a period without a quota limits nothing: {:?}",
        config.options.limits
    );
}

#[test]
fn limits_and_sandbox_survive_a_toml_round_trip() {
    let config = config_from_env(&[
        ("RIKU_ISOLATION_ROOT", "/srv/jail"),
        ("RIKU_ISOLATION_CPU_QUOTA_US", "20000"),
    ]);

    let text = toml::to_string(&config).unwrap();
    let parsed: WorkerConfig = toml::from_str(&text).unwrap();
    let quota = parsed.options.limits.and_then(|limits| limits.cpu_quota_us);
    assert_eq!(quota, Some(20000), "written TOML:\n{text}");
    assert!(parsed.options.sandbox.is_some(), "written TOML:\n{text}");
}

#[test]
fn options_without_timeouts_read_the_defaults() {
    let text = r#"
[worker]
app = "app"
kind = "web"
command = "run"
ordinal = 1

[env]

[options]
working_dir = "/srv/app"
log_file = "/srv/log"
"#;

    let parsed: WorkerConfig = toml::from_str(text).unwrap();

    let options = parsed.options;
    assert_eq!(options.timeout, riku_config::RIKU_WORKER_TIMEOUT);
    assert_eq!(options.grace_period, riku_config::RIKU_WORKER_GRACE_PERIOD);
    assert_eq!(options.max_restarts, riku_config::RIKU_MAX_RESTARTS);
    assert!(options.limits.is_none(), "limits: {:?}", options.limits);
    assert!(options.sandbox.is_none(), "sandbox: {:?}", options.sandbox);
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
