use std::fs;
use std::path::Path;

use super::*;

/// Route destroy through Caddy with a `caddy` that accepts every reload, and
/// serve the router helper's socket from inside the test.
fn use_mock_caddy(paths: &mut RikuPaths) {
    use std::os::unix::fs::PermissionsExt;

    let caddy = paths.riku_root.with_file_name("mock_caddy");
    fs::write(&caddy, "#!/bin/sh\nexit 0\n").unwrap();
    fs::set_permissions(&caddy, fs::Permissions::from_mode(0o755)).unwrap();
    paths.reload_programs.caddy = caddy;
    fs::create_dir_all(&paths.riku_root).unwrap();
    crate::config::router::save(paths, &crate::config::router::Router::Caddy).unwrap();
    riku_route::serve_caddy_for_tests(paths);
}

#[test]
fn removes_the_repo_link_but_not_the_repo_it_points_to() {
    let tmp = tempfile::tempdir().unwrap();
    let mut paths = RikuPaths::for_tests(tmp.path());
    use_mock_caddy(&mut paths);
    let user_repo = tmp.path().join("shop.git");
    fs::create_dir_all(&user_repo).unwrap();
    fs::create_dir_all(paths.app_root.join("shop")).unwrap();
    fs::create_dir_all(paths.git_root.join("shop")).unwrap();
    let link = paths.git_root.join("shop.git");
    std::os::unix::fs::symlink(&user_repo, &link).unwrap();

    cmd_destroy(&paths, "shop", DestroyMode::Safe).unwrap();

    assert!(
        fs::symlink_metadata(&link).is_err(),
        "repo link {} survived destroy",
        link.display()
    );
    assert!(user_repo.exists(), "destroy followed the link");
}

#[test]
fn destroy_removes_the_published_static_files() {
    let tmp = tempfile::tempdir().unwrap();
    let mut paths = RikuPaths::for_tests(tmp.path());
    use_mock_caddy(&mut paths);
    fs::create_dir_all(paths.app_root.join("shop")).unwrap();
    let copy = paths.static_root.join("shop~1");
    fs::create_dir_all(copy.join("root")).unwrap();
    std::os::unix::fs::symlink("shop~1", paths.static_root.join("shop")).unwrap();

    cmd_destroy(&paths, "shop", DestroyMode::Safe).unwrap();

    let left: Vec<_> = fs::read_dir(&paths.static_root).unwrap().collect();
    assert!(
        left.is_empty(),
        "published files survived destroy: {left:?}"
    );
}

#[test]
fn destroy_deletes_the_app_dir_last_so_a_failure_can_be_retried() {
    let tmp = tempfile::tempdir().unwrap();
    let mut paths = RikuPaths::for_tests(tmp.path());
    paths.reload_programs.nginx = tmp.path().join("nginx");
    fs::write(&paths.reload_programs.nginx, "").unwrap();
    use_mock_caddy(&mut paths);
    fs::create_dir_all(paths.app_root.join("shop")).unwrap();
    fs::create_dir_all(paths.env_root.join("shop")).unwrap();
    // A file where the log directory should be: removing it as a directory
    // fails, after the env dir and before the app dir.
    fs::create_dir_all(&paths.log_root).unwrap();
    fs::write(paths.log_root.join("shop"), "not a directory").unwrap();

    let err = cmd_destroy(&paths, "shop", DestroyMode::Safe).unwrap_err();

    assert!(format!("{err:#}").contains("logs/shop"), "{err:#}");
    assert!(
        !paths.env_root.join("shop").exists(),
        "steps before the failure did not run"
    );
    assert!(
        paths.app_root.join("shop").exists(),
        "the app dir went before a later step failed, so a retry cannot find the app"
    );
}

fn container_app_without_engine(root: &Path) -> RikuPaths {
    let mut paths = RikuPaths::for_tests(root);
    use_mock_caddy(&mut paths);
    let app_dir = paths.app_root.join("shop");
    fs::create_dir_all(&app_dir).unwrap();
    fs::write(
        app_dir.join("compose.yaml"),
        "services:\n  web:\n    image: nginx:alpine\n",
    )
    .unwrap();
    fs::create_dir_all(paths.env_root.join("shop")).unwrap();
    fs::write(paths.env_root.join("shop").join("ENV"), "").unwrap();
    fs::create_dir_all(&paths.riku_root).unwrap();
    let engine = root.join("no-such-engine").display().to_string();
    crate::config::settings::save_key(&paths, "engine", "program", Some(&engine)).unwrap();
    paths
}

#[test]
fn destroy_refuses_to_orphan_containers_when_the_engine_is_unreachable() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = container_app_without_engine(tmp.path());

    let err = cmd_destroy(&paths, "shop", DestroyMode::Safe).unwrap_err();

    let message = format!("{err:#}");
    assert!(
        message.contains("unreachable") && message.contains("orphaned"),
        "{message}"
    );
    assert!(
        paths.app_root.join("shop").exists(),
        "a refused destroy removed the app"
    );
}

#[test]
fn forced_destroy_removes_the_app_when_the_engine_is_unreachable() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = container_app_without_engine(tmp.path());

    cmd_destroy(&paths, "shop", DestroyMode::Force).unwrap();

    assert!(
        !paths.app_root.join("shop").exists(),
        "forced destroy left the app directory"
    );
}

fn seed_worker(paths: &RikuPaths, app: &str, kind: &str) -> String {
    let config = riku_supervisor::config::create_worker_config(
        app,
        kind,
        "true",
        1,
        std::collections::HashMap::new(),
        "/srv",
        "/srv/log",
    );
    let name = format!("{app}@{kind}.1.toml");
    let available = paths.workers_available.join(&name);
    fs::create_dir_all(&paths.workers_available).unwrap();
    fs::create_dir_all(&paths.workers_enabled).unwrap();
    fs::write(&available, toml::to_string(&config).unwrap()).unwrap();
    std::os::unix::fs::symlink(&available, paths.workers_enabled.join(&name)).unwrap();
    name
}

#[test]
fn destroy_leaves_worker_configs_of_an_app_whose_name_extends_it() {
    let tmp = tempfile::tempdir().unwrap();
    let mut paths = RikuPaths::for_tests(tmp.path());
    paths.reload_programs.nginx = tmp.path().join("nginx");
    fs::write(&paths.reload_programs.nginx, "").unwrap();
    use_mock_caddy(&mut paths);
    fs::create_dir_all(paths.app_root.join("shop")).unwrap();
    let own = seed_worker(&paths, "shop", "web");
    let other = seed_worker(&paths, "shop-api", "web");

    cmd_destroy(&paths, "shop", DestroyMode::Safe).unwrap();

    for dir in [&paths.workers_available, &paths.workers_enabled] {
        assert!(
            fs::symlink_metadata(dir.join(&own)).is_err(),
            "{own} survived in {}",
            dir.display()
        );
        assert!(
            dir.join(&other).exists(),
            "destroying shop removed {other} from {}",
            dir.display()
        );
    }
}
