use std::fs;
use tempfile::TempDir;

use super::mock_supervisor::{MockRoutes, MockSupervisor};
use super::*;
use crate::util::backend::Backend;

fn make_paths(tmp: &TempDir) -> RikuPaths {
    let paths = crate::config::RikuPaths::for_tests(tmp.path());
    fs::create_dir_all(&paths.workers_available).unwrap();
    fs::create_dir_all(&paths.workers_enabled).unwrap();
    fs::create_dir_all(&paths.nginx_root).unwrap();
    paths
}

/// A deploy whose web copies a mock supervisor runs and a mock router routes.
fn deploy(
    app_path: &Path,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
    start_cmd: Option<&str>,
) -> Result<()> {
    let _supervisor = MockSupervisor::start(paths);
    let mut routes = MockRoutes::new(paths);
    create_workers_with("myapp", app_path, env, paths, start_cmd, &mut routes)
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
fn release_and_preflight_run_once_and_never_become_workers() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    let app_path = tmp.path().join("app");
    fs::create_dir_all(&app_path).unwrap();
    fs::create_dir_all(paths.env_root.join("myapp")).unwrap();
    fs::create_dir_all(paths.log_root.join("myapp")).unwrap();
    fs::write(
        app_path.join("Procfile"),
        "web: python app.py\nrelease: python migrate.py\npreflight: ./check.sh\n",
    )
    .unwrap();

    deploy(&app_path, &HashMap::new(), &paths, None).unwrap();

    for kind in ["release", "preflight"] {
        let config = paths.workers_available.join(format!("myapp@{kind}.1.toml"));
        assert!(!config.exists(), "{kind} got a worker config");
    }
    assert!(paths.workers_available.join("myapp@web.1.toml").exists());
}

#[test]
fn runs_the_plugin_start_command_as_web_without_a_procfile() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    let app_path = tmp.path().join("app");
    fs::create_dir_all(&app_path).unwrap();
    fs::create_dir_all(paths.env_root.join("myapp")).unwrap();
    fs::create_dir_all(paths.log_root.join("myapp")).unwrap();

    let start = "docker compose -f compose.yml up";
    deploy(&app_path, &HashMap::new(), &paths, Some(start)).unwrap();

    let config = fs::read_to_string(paths.workers_available.join("myapp@web.1.toml"))
        .expect("web worker config should be created");
    assert!(
        config.contains(start),
        "config lacks the start command: {config}"
    );
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

    let config_path = paths.workers_available.join("myapp@worker.1.toml");
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

    let symlink_path = paths.workers_enabled.join("myapp@worker.1.toml");
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

    let existing = paths.workers_enabled.join("myapp@web.1.toml");
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

/// Redeploying allocates fresh ports, so what the router is given must be
/// the ports of the latest deploy's copies, every copy of them.
#[test]
fn redeploy_routes_every_copy_of_the_latest_deploy() -> anyhow::Result<()> {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    let app_path = tmp.path().join("app");
    fs::create_dir_all(&app_path).unwrap();
    fs::create_dir_all(paths.env_root.join("myapp")).unwrap();
    fs::create_dir_all(paths.log_root.join("myapp")).unwrap();
    fs::write(paths.env_root.join("myapp/SCALING"), "web=3\n")?;
    fs::write(app_path.join("Procfile"), "web: python app.py\n")?;

    let env = HashMap::new();
    deploy(&app_path, &env, &paths, None)?;
    deploy(&app_path, &env, &paths, None)?;

    let worker_ports: Vec<u16> = crate::router::web_copies("myapp", &paths)?
        .iter()
        .map(|copy| copy.env["PORT"].parse().unwrap())
        .collect();
    assert_eq!(worker_ports.len(), 3, "enabled copies: {worker_ports:?}");
    let routed = crate::router::current_backends("myapp", &env, &paths, None)?;
    let expected: Vec<_> = worker_ports.into_iter().map(Backend::loopback).collect();
    assert_eq!(routed, expected);
    Ok(())
}

#[test]
fn refuses_to_scale_web_past_one_on_a_pinned_port() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    let app_path = tmp.path().join("app");
    fs::create_dir_all(&app_path).unwrap();
    let env_dir = paths.env_root.join("myapp");
    fs::create_dir_all(&env_dir).unwrap();
    fs::create_dir_all(paths.log_root.join("myapp")).unwrap();
    fs::write(env_dir.join("SCALING"), "web=2\n").unwrap();
    fs::write(app_path.join("Procfile"), "web: python app.py\n").unwrap();
    let env = HashMap::from([("PORT".to_string(), "3000".to_string())]);

    let err = create_workers_generic("myapp", &app_path, &env, &paths, None).unwrap_err();

    assert!(err.to_string().contains("3000"), "{err}");
    assert!(!paths.workers_available.join("myapp@web.1.toml").exists());
}

fn pinned_app(tmp: &TempDir) -> (RikuPaths, std::path::PathBuf) {
    let paths = make_paths(tmp);
    let app_path = tmp.path().join("app");
    fs::create_dir_all(&app_path).unwrap();
    fs::create_dir_all(paths.env_root.join("myapp")).unwrap();
    fs::create_dir_all(paths.log_root.join("myapp")).unwrap();
    fs::write(app_path.join("Procfile"), "web: python app.py\n").unwrap();
    (paths, app_path)
}

#[test]
fn custom_socket_refuses_scale() {
    let tmp = TempDir::new().unwrap();
    let (paths, app_path) = pinned_app(&tmp);
    fs::write(paths.env_root.join("myapp/SCALING"), "wsgi=2\n").unwrap();
    fs::write(app_path.join("Procfile"), "wsgi: uwsgi app.ini\n").unwrap();
    let env = HashMap::from([("RIKU_NGINX_SOCKET".to_string(), "/srv/s.sock".to_string())]);

    let err = create_workers_generic("myapp", &app_path, &env, &paths, None).unwrap_err();

    assert!(err.to_string().contains("RIKU_NGINX_SOCKET"), "{err}");
    assert!(!paths.workers_available.join("myapp@wsgi.1.toml").exists());
}

#[test]
fn stale_custom_socket_does_not_stop_a_tcp_web_app_scaling() {
    let tmp = TempDir::new().unwrap();
    let (paths, app_path) = pinned_app(&tmp);
    fs::write(paths.env_root.join("myapp/SCALING"), "web=2\n").unwrap();
    let env = HashMap::from([("RIKU_NGINX_SOCKET".to_string(), "/srv/s.sock".to_string())]);

    deploy(&app_path, &env, &paths, None).unwrap();

    assert!(paths.workers_available.join("myapp@web.2.toml").exists());
}

#[test]
fn refuses_a_pinned_port_another_app_uses() {
    let tmp = TempDir::new().unwrap();
    let (paths, app_path) = pinned_app(&tmp);
    fs::create_dir_all(paths.env_root.join("other")).unwrap();
    fs::write(paths.env_root.join("other/ENV"), "PORT=3000\n").unwrap();
    let env = HashMap::from([("PORT".to_string(), "3000".to_string())]);

    let err = create_workers_generic("myapp", &app_path, &env, &paths, None).unwrap_err();

    assert!(err.to_string().contains("other"), "{err}");
}

#[test]
fn a_stale_upstream_port_in_another_apps_env_is_not_a_claim() {
    let tmp = TempDir::new().unwrap();
    let (paths, _) = pinned_app(&tmp);
    fs::create_dir_all(paths.env_root.join("other")).unwrap();
    fs::write(
        paths.env_root.join("other/ENV"),
        "RIKU_UPSTREAM_PORT=3000\n",
    )
    .unwrap();
    let env = HashMap::from([("PORT".to_string(), "3000".to_string())]);

    pinned::check_pinned_port_with("myapp", &env, &paths, |_| false).unwrap();
}

#[test]
fn refuses_a_pinned_port_something_else_listens_on_unless_it_was_ours() {
    let tmp = TempDir::new().unwrap();
    let (paths, app_path) = pinned_app(&tmp);
    let holder = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = holder.local_addr().unwrap().port().to_string();
    let env = HashMap::from([("PORT".to_string(), port.clone())]);

    let err = create_workers_generic("myapp", &app_path, &env, &paths, None).unwrap_err();
    assert!(err.to_string().contains("in use"), "{err}");

    // On a redeploy the listener is this app's own previous process.
    let previous = HashMap::from([("PORT".to_string(), port)]);
    write_worker_config(
        "myapp",
        &app_path,
        "web",
        "python app.py",
        1,
        previous,
        &paths,
    )
    .unwrap();
    let mut routes = MockRoutes::new(&paths);
    create_workers_with("myapp", &app_path, &env, &paths, None, &mut routes).unwrap();
}
