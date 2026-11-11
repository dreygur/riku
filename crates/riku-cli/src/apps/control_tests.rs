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

/// A deployed `kind` worker of `shop` whose config was last written an hour
/// ago. Returns the config file the enabled link points at.
fn deployed_worker(paths: &RikuPaths, kind: &str) -> PathBuf {
    fs::create_dir_all(paths.app_root.join("shop")).unwrap();
    fs::create_dir_all(&paths.workers_available).unwrap();
    fs::create_dir_all(&paths.workers_enabled).unwrap();
    let name = format!("shop@{kind}.1.toml");
    let config = paths.workers_available.join(&name);
    fs::write(
        &config,
        format!("[worker]\napp = \"shop\"\nkind = \"{kind}\"\n"),
    )
    .unwrap();
    let hour_ago = SystemTime::now() - Duration::from_secs(3600);
    let file = fs::File::options().write(true).open(&config).unwrap();
    file.set_modified(hour_ago).unwrap();
    std::os::unix::fs::symlink(&config, paths.workers_enabled.join(&name)).unwrap();
    config
}

fn modified(config: &PathBuf) -> SystemTime {
    fs::metadata(config).unwrap().modified().unwrap()
}

/// A stand-in supervisor: a process recorded in `supervisor.pid` that a
/// SIGHUP ends, with the file locked as a real supervisor keeps it. The lock
/// lasts as long as the returned file.
fn fake_supervisor(paths: &RikuPaths) -> (std::process::Child, fs::File) {
    let child = Command::new("sleep").arg("30").spawn().unwrap();
    fs::create_dir_all(&paths.riku_root).unwrap();
    let pid_file = paths.riku_root.join("supervisor.pid");
    fs::write(&pid_file, child.id().to_string()).unwrap();
    let lock = fs::File::open(&pid_file).unwrap();
    lock.lock().unwrap();
    (child, lock)
}

#[test]
fn restart_of_a_procfile_app_restarts_its_workers_without_a_build() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    let config = deployed_worker(&paths, "web");
    let (mut supervisor, _lock) = fake_supervisor(&paths);
    let before = modified(&config);

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
    assert!(
        modified(&config) > before,
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

#[test]
fn restart_of_one_process_kind_leaves_the_other_kinds_running() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    let web = deployed_worker(&paths, "web");
    let worker = deployed_worker(&paths, "worker");
    let web_before = modified(&web);
    let worker_before = modified(&worker);
    let (mut supervisor, _lock) = fake_supervisor(&paths);

    let restarted = cmd_restart(&paths, "shop", Some("worker"));

    let status = supervisor.wait().unwrap();
    assert!(restarted.is_ok(), "restart failed: {restarted:?}");
    assert!(
        modified(&worker) > worker_before,
        "the worker kind's config mtime must advance"
    );
    assert_eq!(modified(&web), web_before, "web must not be restarted");
    assert_eq!(
        status.signal(),
        Some(nix::sys::signal::Signal::SIGHUP as i32),
        "the supervisor must get a SIGHUP to reload, got {status:?}"
    );
}

#[test]
fn restart_of_an_unknown_process_kind_names_the_apps_kinds() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    let web = deployed_worker(&paths, "web");
    deployed_worker(&paths, "worker");
    let before = modified(&web);

    let restarted = cmd_restart(&paths, "shop", Some("db"));

    let message = format!("{:#}", restarted.expect_err("shop runs no db kind"));
    assert!(
        message.contains("'db'") && message.contains("web, worker"),
        "the error must name the unknown kind and the app's kinds, got: {message}"
    );
    assert_eq!(modified(&web), before, "nothing may restart on an error");
}

#[test]
fn stop_refuses_a_process_name_for_a_procfile_app() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    deployed_worker(&paths, "web");

    let stopped = cmd_stop(&paths, "shop", Some("web"));

    let message = format!("{:#}", stopped.expect_err("stop takes no kind"));
    assert!(
        message.contains("not a container app"),
        "the error must say why the name is refused, got: {message}"
    );
    assert!(
        paths.workers_enabled.join("shop@web.1.toml").exists(),
        "a refused stop must leave the worker enabled"
    );
}
