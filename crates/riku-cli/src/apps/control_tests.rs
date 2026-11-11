use std::fs;
use std::os::unix::process::ExitStatusExt;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

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

/// A deployed web worker of `shop` whose config was last written an hour
/// ago. Returns the config file the enabled link points at.
fn deployed_worker(paths: &RikuPaths) -> PathBuf {
    fs::create_dir_all(paths.app_root.join("shop")).unwrap();
    fs::create_dir_all(&paths.workers_available).unwrap();
    fs::create_dir_all(&paths.workers_enabled).unwrap();
    let config = paths.workers_available.join("shop@web.1.toml");
    fs::write(&config, "[worker]\napp = \"shop\"\nkind = \"web\"\n").unwrap();
    let hour_ago = SystemTime::now() - Duration::from_secs(3600);
    let file = fs::File::options().write(true).open(&config).unwrap();
    file.set_modified(hour_ago).unwrap();
    std::os::unix::fs::symlink(&config, paths.workers_enabled.join("shop@web.1.toml")).unwrap();
    config
}

/// A stand-in supervisor: a process recorded in `supervisor.pid` that a
/// SIGHUP ends.
fn fake_supervisor(paths: &RikuPaths) -> std::process::Child {
    let child = Command::new("sleep").arg("30").spawn().unwrap();
    fs::create_dir_all(&paths.riku_root).unwrap();
    fs::write(
        paths.riku_root.join("supervisor.pid"),
        child.id().to_string(),
    )
    .unwrap();
    child
}

#[test]
fn restart_of_a_procfile_app_restarts_its_workers_without_a_build() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    let config = deployed_worker(&paths);
    let mut supervisor = fake_supervisor(&paths);
    let before = fs::metadata(&config).unwrap().modified().unwrap();

    let restarted = cmd_restart(&paths, "shop", None);

    let status = supervisor.wait().unwrap();
    assert!(restarted.is_ok(), "restart failed: {restarted:?}");
    assert!(
        !paths.deploy_log_file("shop").exists(),
        "restart must not run a deploy, but it opened the deploy log"
    );
    assert!(
        paths.workers_enabled.join("shop@web.1.toml").exists(),
        "restart must keep the worker enabled"
    );
    let after = fs::metadata(&config).unwrap().modified().unwrap();
    assert!(
        after > before,
        "the worker config mtime must advance so the supervisor reloads it"
    );
    assert_eq!(
        status.signal(),
        Some(nix::sys::signal::Signal::SIGHUP as i32),
        "the supervisor must get a SIGHUP to reload, got {status:?}"
    );
}

#[test]
fn restart_of_a_procfile_app_with_no_workers_names_deploy() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    fs::create_dir_all(paths.app_root.join("shop")).unwrap();

    let restarted = cmd_restart(&paths, "shop", None);

    let message = format!(
        "{:#}",
        restarted.expect_err("nothing runs, so nothing to restart")
    );
    assert!(
        message.contains("riku deploy shop"),
        "the error must say how to start the app, got: {message}"
    );
}
