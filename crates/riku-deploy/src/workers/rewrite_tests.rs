use std::collections::HashMap;
use std::fs;

use riku_supervisor::config::WorkerConfig;

use super::{kinds_down_while_restarting, rewrite_running};
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

/// Every enabled config's text, by file name.
fn config_texts(paths: &RikuPaths) -> Vec<(String, String)> {
    let mut texts: Vec<(String, String)> = fs::read_dir(&paths.workers_enabled)
        .unwrap()
        .flatten()
        .map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            (name, fs::read_to_string(entry.path()).unwrap())
        })
        .collect();
    texts.sort();
    texts
}

/// `myapp` deployed with one copy of each of `kinds` while its env said
/// `FLAG=old`, on a mock supervisor and router.
fn deployed(paths: &RikuPaths, kinds: &[&str]) -> (MockSupervisor, MockRoutes, std::path::PathBuf) {
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
    let processes: HashMap<String, String> = kinds
        .iter()
        .map(|kind| (kind.to_string(), format!("python {kind}.py")))
        .collect();
    let supervisor = MockSupervisor::start(paths);
    let old = HashMap::from([("FLAG".to_string(), "old".to_string())]);
    let mut routes = MockRoutes::new(paths);
    create_workers_with(
        "myapp",
        &app_path,
        Some(&processes),
        &old,
        paths,
        None,
        &mut routes,
    )
    .unwrap();
    (supervisor, routes, app_path)
}

/// A web copy and a worker, deployed with `FLAG=old`, then rewritten with
/// `FLAG=new`: every config carries the new value, the web kind keeps its
/// copy count and the router was given the new copy before the old one left.
#[test]
fn rewrite_gives_every_running_copy_the_new_env_and_keeps_web_routed() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    let (supervisor, mut routes, app_path) = deployed(&paths, &["web", "worker"]);
    let published_by_deploy = routes.published.len();

    let new = HashMap::from([("FLAG".to_string(), "new".to_string())]);
    let rewritten = rewrite_running(
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

/// `beat` sorts before `web`, so it used to be rewritten before the router
/// was asked; a router helper that is down must leave every config as it was.
#[test]
fn a_router_that_would_refuse_changes_no_config() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    let (supervisor, mut routes, app_path) = deployed(&paths, &["beat", "web"]);
    let before = config_texts(&paths);
    routes.refuse_check = true;

    let new = HashMap::from([("FLAG".to_string(), "new".to_string())]);
    let rewritten = rewrite_running(
        "myapp",
        &app_path,
        &enabled(&paths),
        &new,
        &paths,
        &mut routes,
    );

    assert!(rewritten.is_err(), "a refused router check must fail");
    assert_eq!(config_texts(&paths), before, "no config may change");
    drop(supervisor);
}

/// The router does not confirm the new web copy once: the old routes are
/// published again and the new copy stopped, so a later restart, which
/// counts the copies it finds, does not keep one too many.
#[test]
fn an_unconfirmed_publish_leaves_exactly_the_wanted_copies() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    let (supervisor, mut routes, app_path) = deployed(&paths, &["web"]);
    routes.fail_publish_after(1);

    let new = HashMap::from([("FLAG".to_string(), "new".to_string())]);
    let rewritten = rewrite_running(
        "myapp",
        &app_path,
        &enabled(&paths),
        &new,
        &paths,
        &mut routes,
    );

    assert!(rewritten.is_err(), "an unconfirmed publish must fail");
    let web: Vec<WorkerConfig> = enabled(&paths);
    assert_eq!(
        web.len(),
        1,
        "exactly the one wanted web copy stays: {:?}",
        web.iter().map(|c| c.worker.name()).collect::<Vec<_>>()
    );
    assert_eq!(
        web[0].env.get("FLAG").map(String::as_str),
        Some("old"),
        "the copy left is the old one, still routed"
    );
    drop(supervisor);
}

/// A pinned `PORT` stops the old copy before its replacement starts, so
/// restart must say the route goes down.
#[test]
fn only_stop_first_web_kinds_lose_their_route_while_restarting() {
    let kinds = ["web", "worker", "wsgi"];
    let pinned = HashMap::from([("PORT".to_string(), "8000".to_string())]);

    assert_eq!(
        kinds_down_while_restarting(kinds, &HashMap::new()).unwrap(),
        Vec::<&str>::new(),
        "overlapping copies keep the route up"
    );
    assert_eq!(
        kinds_down_while_restarting(kinds, &pinned).unwrap(),
        ["web"]
    );
}
