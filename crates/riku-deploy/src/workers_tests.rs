use super::*;
use tempfile::TempDir;

fn make_paths(tmp: &TempDir) -> RikuPaths {
    let paths = crate::config::RikuPaths::for_tests(tmp.path());
    fs::create_dir_all(&paths.workers_available).unwrap();
    fs::create_dir_all(&paths.workers_enabled).unwrap();
    fs::create_dir_all(&paths.nginx_root).unwrap();
    paths
}

// --- read_scaling_count ---

#[test]
fn test_read_scaling_count_default_when_no_file() -> anyhow::Result<()> {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    fs::create_dir_all(paths.env_root.join("myapp")).unwrap();
    let count = read_scaling_count(&paths, "myapp", "web")?;
    assert_eq!(count, 1, "Default scaling count should be 1");
    Ok(())
}

#[test]
fn test_read_scaling_count_reads_file() -> anyhow::Result<()> {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    let env_dir = paths.env_root.join("myapp");
    fs::create_dir_all(&env_dir).unwrap();
    fs::write(env_dir.join("SCALING"), "web=3\nworker=2\n")?;
    assert_eq!(read_scaling_count(&paths, "myapp", "web")?, 3);
    assert_eq!(read_scaling_count(&paths, "myapp", "worker")?, 2);
    Ok(())
}

#[test]
fn test_read_scaling_count_unknown_kind_returns_one() -> anyhow::Result<()> {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    let env_dir = paths.env_root.join("myapp");
    fs::create_dir_all(&env_dir).unwrap();
    fs::write(env_dir.join("SCALING"), "web=2\n")?;
    assert_eq!(read_scaling_count(&paths, "myapp", "cron")?, 1);
    Ok(())
}

#[test]
fn test_read_scaling_count_ignores_comments() -> anyhow::Result<()> {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    let env_dir = paths.env_root.join("myapp");
    fs::create_dir_all(&env_dir).unwrap();
    fs::write(env_dir.join("SCALING"), "# web=99\nweb=1\n")?;
    assert_eq!(read_scaling_count(&paths, "myapp", "web")?, 1);
    Ok(())
}

// --- create_workers_generic ---

#[test]
fn test_create_workers_generic_no_procfile_returns_ok() -> anyhow::Result<()> {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    let app_path = tmp.path().join("app");
    fs::create_dir_all(&app_path).unwrap();
    fs::create_dir_all(paths.env_root.join("myapp")).unwrap();
    fs::create_dir_all(paths.log_root.join("myapp")).unwrap();

    let env = HashMap::new();
    create_workers_generic("myapp", &app_path, &env, &paths, None)
}

#[test]
fn test_create_workers_generic_worker_kind_creates_config() -> anyhow::Result<()> {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    let app_path = tmp.path().join("app");
    fs::create_dir_all(&app_path).unwrap();
    fs::create_dir_all(paths.env_root.join("myapp")).unwrap();
    fs::create_dir_all(paths.log_root.join("myapp")).unwrap();

    fs::write(app_path.join("Procfile"), "worker: python worker.py\n")?;

    let env = HashMap::new();
    create_workers_generic("myapp", &app_path, &env, &paths, None)?;

    let config_path = paths.workers_available.join("myapp-worker-1.toml");
    assert!(config_path.exists(), "worker config should be created");
    Ok(())
}

#[test]
fn test_create_workers_generic_symlink_created_in_enabled() -> anyhow::Result<()> {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    let app_path = tmp.path().join("app");
    fs::create_dir_all(&app_path).unwrap();
    fs::create_dir_all(paths.env_root.join("myapp")).unwrap();
    fs::create_dir_all(paths.log_root.join("myapp")).unwrap();

    fs::write(app_path.join("Procfile"), "worker: python worker.py\n")?;

    let env = HashMap::new();
    create_workers_generic("myapp", &app_path, &env, &paths, None)?;

    let symlink_path = paths.workers_enabled.join("myapp-worker-1.toml");
    assert!(
        symlink_path.exists(),
        "symlink in workers_enabled should exist"
    );
    Ok(())
}

#[test]
fn test_create_workers_generic_skips_comment_lines() -> anyhow::Result<()> {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    let app_path = tmp.path().join("app");
    fs::create_dir_all(&app_path).unwrap();
    fs::create_dir_all(paths.env_root.join("myapp")).unwrap();
    fs::create_dir_all(paths.log_root.join("myapp")).unwrap();

    fs::write(app_path.join("Procfile"), "# comment\nworker: echo hello\n")?;

    let env = HashMap::new();
    create_workers_generic("myapp", &app_path, &env, &paths, None)?;

    let entries: Vec<_> = fs::read_dir(&paths.workers_available)
        .unwrap()
        .flatten()
        .collect();
    assert_eq!(entries.len(), 1);
    Ok(())
}

#[test]
fn test_auto_restart_false_skips_removal_of_existing_configs() -> anyhow::Result<()> {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    let app_path = tmp.path().join("app");
    fs::create_dir_all(&app_path).unwrap();
    fs::create_dir_all(paths.env_root.join("myapp")).unwrap();
    fs::create_dir_all(paths.log_root.join("myapp")).unwrap();

    let existing = paths.workers_enabled.join("myapp-web-1.toml");
    fs::write(&existing, "[worker]\n")?;

    fs::write(app_path.join("Procfile"), "worker: echo hello\n")?;

    let mut env = HashMap::new();
    env.insert("RIKU_AUTO_RESTART".to_string(), "false".to_string());
    create_workers_generic("myapp", &app_path, &env, &paths, None)?;

    assert!(
        existing.exists(),
        "existing config should be preserved when RIKU_AUTO_RESTART=false"
    );
    Ok(())
}

/// Regression test for the stale-NGINX_INTERNAL_PORT bug: every deploy
/// allocates a fresh ephemeral port for a `web` worker, so the ENV
/// file's NGINX_INTERNAL_PORT (read back by `spawn_app`'s nginx config
/// generation) must track the latest deploy's port, not just the
/// first one ever persisted.
#[test]
fn test_redeploy_refreshes_persisted_nginx_internal_port() -> anyhow::Result<()> {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    let app_path = tmp.path().join("app");
    fs::create_dir_all(&app_path).unwrap();
    fs::create_dir_all(paths.env_root.join("myapp")).unwrap();
    fs::create_dir_all(paths.log_root.join("myapp")).unwrap();
    fs::write(app_path.join("Procfile"), "web: python app.py\n")?;

    let env = HashMap::new();
    create_workers_generic("myapp", &app_path, &env, &paths, None)?;

    let env_file = paths.env_root.join("myapp").join("ENV");
    let mut persisted = HashMap::new();
    crate::util::parse_settings(&env_file, &mut persisted)?;
    let first_port = persisted
        .get("NGINX_INTERNAL_PORT")
        .expect("first deploy should persist NGINX_INTERNAL_PORT")
        .clone();

    // Second deploy ("redeploy"): a fresh port gets allocated.
    create_workers_generic("myapp", &app_path, &env, &paths, None)?;

    let worker_toml = fs::read_to_string(paths.workers_available.join("myapp-web-1.toml"))?;
    let actual_port = worker_toml
        .lines()
        .find_map(|l| l.strip_prefix("PORT = \""))
        .and_then(|rest| rest.strip_suffix('"'))
        .expect("worker config should contain a PORT value")
        .to_string();

    let mut persisted_after = HashMap::new();
    crate::util::parse_settings(&env_file, &mut persisted_after)?;
    let second_port = persisted_after
        .get("NGINX_INTERNAL_PORT")
        .expect("redeploy should still have NGINX_INTERNAL_PORT")
        .clone();

    assert_eq!(
            second_port, actual_port,
            "persisted NGINX_INTERNAL_PORT must match the worker actually spawned by the redeploy, not a stale port from the first deploy"
        );
    let _ = first_port; // not asserted equal/different: ports are random, could coincidentally collide
    Ok(())
}
