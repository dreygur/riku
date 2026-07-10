use super::*;
use tempfile::TempDir;

fn make_paths(tmp: &TempDir) -> RikuPaths {
    let paths = RikuPaths::for_tests(tmp.path());
    for dir in &[
        &paths.app_root,
        &paths.env_root,
        &paths.nginx_root,
        &paths.riku_root,
    ] {
        fs::create_dir_all(dir).unwrap();
    }
    paths
}

#[test]
fn test_extract_routing_fields_keeps_only_allowlisted_keys() {
    let mut env = HashMap::new();
    env.insert("PORT".to_string(), "5000".to_string());
    env.insert("DATABASE_URL".to_string(), "postgres://secret".to_string());
    env.insert("SECRET_KEY".to_string(), "abc123".to_string());
    env.insert("NGINX_INTERNAL_PORT".to_string(), "5000".to_string());

    let routing = extract_routing_fields(&env);

    assert_eq!(routing.len(), 2);
    assert_eq!(routing.get("PORT").map(String::as_str), Some("5000"));
    assert_eq!(
        routing.get("NGINX_INTERNAL_PORT").map(String::as_str),
        Some("5000")
    );
    assert!(
        !routing.contains_key("DATABASE_URL"),
        "secret env vars must never appear in routing output"
    );
    assert!(
        !routing.contains_key("SECRET_KEY"),
        "secret env vars must never appear in routing output"
    );
}

#[test]
fn test_extract_routing_fields_empty_env_returns_empty_map() {
    assert!(extract_routing_fields(&HashMap::new()).is_empty());
}

#[test]
fn test_build_app_entry_never_leaks_secret_env_vars() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);

    let app = "secretapp";
    fs::create_dir_all(paths.app_root.join(app)).unwrap();
    let env_dir = paths.env_root.join(app);
    fs::create_dir_all(&env_dir).unwrap();
    fs::write(
        env_dir.join("ENV"),
        "PORT=8080\nDATABASE_URL=postgres://user:pass@host/db\nAPI_TOKEN=topsecret\n",
    )
    .unwrap();

    let entry = build_app_entry(app, &paths, &HashMap::new());
    let serialized = serde_json::to_string(&entry).unwrap();

    assert!(serialized.contains("8080"), "PORT should be present");
    assert!(
        !serialized.contains("postgres"),
        "DATABASE_URL must not appear anywhere in the dump: {}",
        serialized
    );
    assert!(
        !serialized.contains("topsecret"),
        "API_TOKEN must not appear anywhere in the dump: {}",
        serialized
    );
}

#[test]
fn test_build_app_entry_reports_lock_state() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    let app = "lockedapp";
    fs::create_dir_all(paths.app_root.join(app)).unwrap();
    fs::create_dir_all(paths.env_root.join(app)).unwrap();

    let free_entry = build_app_entry(app, &paths, &HashMap::new());
    assert!(matches!(free_entry.deploy_lock, LockState::Free));

    let _held = crate::deploy::lock::acquire(app, &paths).unwrap();
    let held_entry = build_app_entry(app, &paths, &HashMap::new());
    assert!(matches!(held_entry.deploy_lock, LockState::Held));
}

#[test]
fn test_build_app_entry_nginx_state() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    let app = "webapp";
    fs::create_dir_all(paths.app_root.join(app)).unwrap();
    fs::create_dir_all(paths.env_root.join(app)).unwrap();

    let before = build_app_entry(app, &paths, &HashMap::new());
    assert!(!before.nginx.config_exists);

    fs::write(paths.nginx_root.join(format!("{}.conf", app)), "# conf").unwrap();
    let after = build_app_entry(app, &paths, &HashMap::new());
    assert!(after.nginx.config_exists);
}

#[test]
fn test_build_app_entry_includes_workers_from_stats() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    let app = "workedapp";
    fs::create_dir_all(paths.app_root.join(app)).unwrap();
    fs::create_dir_all(paths.env_root.join(app)).unwrap();

    let stats_json = format!(
        r#"[{{"app":"{app}","total_processes":1,"running_processes":1,"healthy_processes":1,
                "total_restarts":0,"total_memory_bytes":0,"total_cpu_time_ms":0,
                "processes":[{{"process_id":"{app}-web-1","app":"{app}","kind":"web","ordinal":1,
                "pid":1234,"status":"running","started_at":null,"last_health_check":null,
                "health_check_status":"unknown","restart_count":0,"last_restart_at":null,
                "cpu_time_ms":0,"memory_bytes":0,"requests_total":0,"requests_per_second":0.0}}],
                "last_updated":"2024-01-01T00:00:00Z"}}]"#,
        app = app
    );
    fs::write(paths.riku_root.join("stats.json"), stats_json).unwrap();

    let app_stats = load_app_stats(&paths);
    let entry = build_app_entry(app, &paths, &app_stats);

    assert_eq!(entry.workers.len(), 1);
    assert_eq!(entry.workers[0].pid, Some(1234));
    assert_eq!(entry.workers[0].process_id, format!("{}-web-1", app));
}

#[test]
fn test_load_app_stats_missing_file_returns_empty() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    assert!(load_app_stats(&paths).is_empty());
}

#[test]
fn test_supervisor_uptime_seconds_missing_pid_file_returns_none() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    assert!(supervisor_uptime_seconds(&paths).is_none());
}

#[test]
fn test_supervisor_uptime_seconds_present() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    fs::write(paths.riku_root.join("supervisor.pid"), "1\n").unwrap();
    let uptime = supervisor_uptime_seconds(&paths);
    assert!(uptime.is_some());
}

#[test]
fn test_build_state_dump_end_to_end_no_panic() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    let app = "fullapp";
    fs::create_dir_all(paths.app_root.join(app)).unwrap();
    fs::create_dir_all(paths.env_root.join(app)).unwrap();
    fs::write(
        paths.env_root.join(app).join("ENV"),
        "PORT=3000\nSECRET=hide\n",
    )
    .unwrap();

    let dump = build_state_dump(&paths).unwrap();
    let serialized = serde_json::to_string(&dump).unwrap();

    assert_eq!(dump.apps.len(), 1);
    assert!(serialized.contains("3000"));
    assert!(!serialized.contains("hide"));
}
