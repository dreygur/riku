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
