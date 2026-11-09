use std::fs;

use super::*;

#[test]
fn run_reports_a_failing_command() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    fs::create_dir_all(paths.app_root.join("shop")).unwrap();

    let result = cmd_run(&paths, "shop", &["false".to_string()]);
    assert!(
        result.is_err(),
        "riku run reported success for a command that failed"
    );
}

#[test]
fn run_succeeds_for_a_passing_command() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    fs::create_dir_all(paths.app_root.join("shop")).unwrap();

    let result = cmd_run(&paths, "shop", &["true".to_string()]);
    assert!(result.is_ok(), "riku run failed: {result:?}");
}
