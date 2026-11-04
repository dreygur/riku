use std::collections::HashMap;

use super::*;
use crate::doctor::Status;

#[test]
fn doctor_reports_legacy_app() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    std::fs::create_dir_all(&paths.workers_enabled).unwrap();
    let worker = riku_supervisor::config::create_worker_config(
        "myapp",
        "web",
        "docker compose -f compose.yaml up",
        1,
        HashMap::new(),
        "/tmp/myapp",
        "/tmp/myapp.log",
    );
    let text = toml::to_string(&worker).unwrap();
    std::fs::write(paths.workers_enabled.join("myapp@web.1.toml"), text).unwrap();

    let check = legacy_containers(&paths);

    assert_eq!(
        check.status,
        Status::Warn,
        "status should be Warn: {}",
        check.detail
    );
    assert!(
        check.detail.contains("myapp"),
        "detail should contain myapp: {}",
        check.detail
    );
}

#[test]
fn no_legacy_apps_is_ok() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    std::fs::create_dir_all(&paths.workers_enabled).unwrap();
    let check = legacy_containers(&paths);
    assert_eq!(check.status, Status::Ok, "{}", check.detail);
    assert_eq!(check.detail, "none");
}
