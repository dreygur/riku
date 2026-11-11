use std::collections::HashMap;
use std::fs;

use riku_supervisor::config::WorkerConfig;

use super::{kinds_down_while_restarting, rewrite_running_with};
use crate::config::RikuPaths;
use crate::workers::create_workers_with;
use crate::workers::mock_supervisor::{MockRoutes, MockSupervisor};

fn enabled(paths: &RikuPaths) -> Vec<WorkerConfig> {
    let mut configs: Vec<WorkerConfig> = fs::read_dir(&paths.workers_enabled)
        .unwrap()
        .flatten()
        .map(|entry| toml::from_str(&fs::read_to_string(entry.path()).unwrap()).unwrap())
        .collect();
    configs.sort_by_key(|config| (config.worker.kind.clone(), config.worker.ordinal));
    configs
}

/// A web copy and a worker, deployed with `FLAG=old`, then rewritten with
/// `FLAG=new`: every config carries the new value, the web kind keeps its
/// copy count and the router was given the new copy before the old one left.
#[test]
fn rewrite_gives_every_running_copy_the_new_env_and_keeps_web_routed() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    for dir in [
        &paths.workers_available,
        &paths.workers_enabled,
        &paths.nginx_root,
    ] {
        fs::create_dir_all(dir).unwrap();
    }
    fs::create_dir_all(paths.env_root.join("myapp")).unwrap();
    fs::create_dir_all(paths.log_root.join("myapp")).unwrap();
    let app_path = paths.app_root.join("myapp");
    fs::create_dir_all(&app_path).unwrap();
    let processes = HashMap::from([
        ("web".to_string(), "python app.py".to_string()),
        ("worker".to_string(), "python jobs.py".to_string()),
    ]);
    let supervisor = MockSupervisor::start(&paths);
    let old = HashMap::from([("FLAG".to_string(), "old".to_string())]);
    let mut routes = MockRoutes::new(&paths);
    create_workers_with(
        "myapp",
        &app_path,
        Some(&processes),
        &old,
        &paths,
        None,
        &mut routes,
    )
    .unwrap();
    let published_by_deploy = routes.published.len();

    let new = HashMap::from([("FLAG".to_string(), "new".to_string())]);
    let rewritten = rewrite_running_with(
        "myapp",
        &app_path,
        &enabled(&paths),
        &new,
        &paths,
        &mut routes,
    );

    assert!(rewritten.is_ok(), "rewrite failed: {rewritten:?}");
    let after = enabled(&paths);
    let kinds: Vec<&str> = after.iter().map(|c| c.worker.kind.as_str()).collect();
    assert_eq!(kinds, ["web", "worker"], "one copy of each kind stays");
    for config in &after {
        assert_eq!(
            config.env.get("FLAG").map(String::as_str),
            Some("new"),
            "{} kept the old env",
            config.worker.name()
        );
    }
    assert!(
        routes.published.len() > published_by_deploy,
        "the web kind must be published to the router again"
    );
    assert!(
        routes.published[published_by_deploy..]
            .iter()
            .all(|(backends, _)| !backends.is_empty()),
        "the router must never be left without a web copy: {:?}",
        routes.published
    );
    drop(supervisor);
}

/// A pinned `PORT` or a shared wsgi socket stops the old copy before its
/// replacement starts, so restart must say the route goes down.
#[test]
fn only_stop_first_web_kinds_lose_their_route_while_restarting() {
    let kinds = ["web", "worker", "wsgi"];
    let pinned = HashMap::from([("PORT".to_string(), "8000".to_string())]);
    let shared_socket = HashMap::from([(
        "RIKU_NGINX_SOCKET".to_string(),
        "/run/riku-sockets/app.sock".to_string(),
    )]);

    assert_eq!(
        kinds_down_while_restarting(kinds, &HashMap::new()).unwrap(),
        Vec::<&str>::new(),
        "overlapping copies keep the route up"
    );
    assert_eq!(
        kinds_down_while_restarting(kinds, &pinned).unwrap(),
        ["web"]
    );
    assert_eq!(
        kinds_down_while_restarting(kinds, &shared_socket).unwrap(),
        ["wsgi"]
    );
}
