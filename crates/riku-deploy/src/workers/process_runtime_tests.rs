use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use riku_supervisor::config::WorkerConfig;
use riku_util::backend::Backend;
use tempfile::TempDir;

use crate::config::RikuPaths;
use crate::workers::create_workers_with;
use crate::workers::mock_supervisor::{MockRoutes, MockSupervisor};

fn web_app(tmp: &TempDir) -> (RikuPaths, PathBuf) {
    let paths = RikuPaths::for_tests(tmp.path());
    let app_path = tmp.path().join("app");
    for dir in [
        &paths.workers_available,
        &paths.workers_enabled,
        &paths.nginx_root,
        &paths.env_root.join("myapp"),
        &paths.log_root.join("myapp"),
        &app_path,
    ] {
        fs::create_dir_all(dir).unwrap();
    }
    fs::write(app_path.join("Procfile"), "web: python app.py\n").unwrap();
    (paths, app_path)
}

fn enabled_web(paths: &RikuPaths) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(&paths.workers_enabled)
        .unwrap()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("myapp@web.") && name.ends_with(".toml"))
        .collect();
    names.sort();
    names
}

fn deploy_twice(
    paths: &RikuPaths,
    app_path: &Path,
    env: &HashMap<String, String>,
) -> (usize, MockRoutes) {
    let supervisor = MockSupervisor::start(paths);
    let mut routes = MockRoutes::new(paths);
    for _ in 0..2 {
        create_workers_with("myapp", app_path, env, paths, None, &mut routes).unwrap();
    }
    (supervisor.max_enabled_web(), routes)
}

#[test]
fn web_copy_overlaps_on_deploy() {
    let tmp = TempDir::new().unwrap();
    let (paths, app_path) = web_app(&tmp);

    let (max_web, routes) = deploy_twice(&paths, &app_path, &HashMap::new());

    let published = &routes.published;
    assert_eq!(
        max_web, 2,
        "old and new copy never ran together; published: {published:?}"
    );
    assert!(
        published.iter().all(|(backends, _)| !backends.is_empty()),
        "the router was given no backend: {published:?}"
    );
    let enabled = enabled_web(&paths);
    assert_eq!(
        enabled,
        ["myapp@web.2.toml"],
        "the new copy should take the free slot"
    );
    assert!(
        !paths.workers_available.join("myapp@web.1.toml").exists(),
        "the replaced copy's config stayed in workers-available"
    );
    let text = fs::read_to_string(paths.workers_enabled.join(&enabled[0])).unwrap();
    let config: WorkerConfig = toml::from_str(&text).unwrap();
    let port: u16 = config.env["PORT"].parse().unwrap();
    assert_eq!(
        published.last().map(|(backends, _)| backends.clone()),
        Some(vec![Backend::loopback(port)]),
        "the last publish should route to the new copy only: {published:?}"
    );
}

#[test]
fn a_stopped_wsgi_copy_leaves_no_socket_behind() {
    let tmp = TempDir::new().unwrap();
    let (paths, app_path) = web_app(&tmp);
    fs::write(app_path.join("Procfile"), "wsgi: app:application\n").unwrap();

    deploy_twice(&paths, &app_path, &HashMap::new());

    let mut sockets: Vec<String> = fs::read_dir(&paths.nginx_root)
        .unwrap()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".sock"))
        .collect();
    sockets.sort();
    assert_eq!(
        sockets,
        ["myapp@2.sock"],
        "the replaced copy's socket stayed"
    );
}

/// A pinned PORT cannot be held by two copies, so the old copy stops before
/// its replacement starts.
#[test]
fn pinned_port_keeps_stop_first() {
    let tmp = TempDir::new().unwrap();
    let (paths, app_path) = web_app(&tmp);
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let env = HashMap::from([("PORT".to_string(), port.to_string())]);

    let (max_web, routes) = deploy_twice(&paths, &app_path, &env);

    assert_eq!(
        max_web, 1,
        "two copies ran on one pinned port; published: {:?}",
        routes.published
    );
    assert_eq!(enabled_web(&paths), ["myapp@web.1.toml"]);
}
