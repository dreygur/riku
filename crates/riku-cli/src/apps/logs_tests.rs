use std::fs;
use tempfile::TempDir;

use super::*;
use crate::config::RikuPaths;

#[test]
fn logs_unknown_service_lists_known() {
    let tmp = TempDir::new().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    let app = "shop";
    let app_log_dir = paths.log_root.join(app);
    fs::create_dir_all(&app_log_dir).unwrap();
    fs::create_dir_all(paths.app_root.join(app)).unwrap();

    fs::write(app_log_dir.join("web.1.log"), "line 1\n").unwrap();
    fs::write(app_log_dir.join("worker.1.log"), "line 2\n").unwrap();

    let err = cmd_logs(&paths, app, "database").unwrap_err().to_string();
    assert!(err.contains("unknown service 'database'"));
    assert!(err.contains("web"));
    assert!(err.contains("worker"));
}

#[test]
fn refuses_service_names_that_would_widen_the_glob() {
    let tmp = TempDir::new().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    fs::create_dir_all(paths.app_root.join("shop")).unwrap();
    fs::create_dir_all(paths.log_root.join("shop")).unwrap();
    fs::create_dir_all(paths.log_root.join("other")).unwrap();
    fs::write(paths.log_root.join("other").join("web.1.log"), "secret\n").unwrap();

    for service in ["../other", "a*"] {
        let err = cmd_logs(&paths, "shop", service).unwrap_err().to_string();
        assert!(
            err.contains("invalid service name"),
            "{service:?} was not refused: {err}"
        );
    }
}

#[test]
fn plain_service_name_is_accepted_and_star_means_all() {
    let web = requested_service("web").unwrap();
    assert_eq!(web.as_ref().map(ServiceName::as_str), Some("web"));
    assert!(
        requested_service("*").unwrap().is_none(),
        "'*' means every service"
    );
    assert!(
        requested_service("").unwrap().is_none(),
        "empty means every service"
    );
}
