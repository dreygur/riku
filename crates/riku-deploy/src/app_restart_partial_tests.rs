use std::cell::Cell;
use std::collections::HashMap;
use std::fs;

use super::env_tests::deployed_app;
use super::*;
use crate::workers::mock_supervisor::{MockRoutes, MockSupervisor};

fn enabled_web_flag(paths: &RikuPaths) -> Option<String> {
    app_worker_configs(&paths.workers_enabled, "myapp")
        .into_iter()
        .filter_map(|path| fs::read_to_string(path).ok())
        .filter_map(|text| toml::from_str::<WorkerConfig>(&text).ok())
        .find(|config| config.worker.kind == "web")
        .and_then(|config| config.env.get("FLAG").cloned())
}

/// Web copy 1 rolls onto the new env, then copy 2's publish is refused.
/// One copy runs the new env, so `LIVE_ENV` must follow it and the error
/// must name the copy that changed.
#[test]
fn one_rolled_web_copy_of_two_writes_live_env_and_is_named() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    deployed_app(&paths);
    let app_path = paths.app_root.join("myapp");
    let mock_supervisor = MockSupervisor::start(&paths);
    let mut routes = MockRoutes::new(&paths);
    fs::write(paths.env_root.join("myapp").join("SCALING"), "web=2\n").unwrap();
    let processes = HashMap::from([("web".to_string(), "python app.py".to_string())]);
    let old = HashMap::from([("FLAG".to_string(), "old".to_string())]);
    workers::create_workers_with(
        "myapp",
        &app_path,
        Some(&processes),
        &old,
        &paths,
        None,
        &mut routes,
    )
    .unwrap();
    fs::write(paths.env_root.join("myapp").join("ENV"), "FLAG=new\n").unwrap();
    // Copy 1 publishes twice (added, then the old one drained); the third
    // publish adds copy 2's replacement.
    routes.fail_publish_after(3);

    let restarted =
        restart_procfile_app_with("myapp", Some("web"), &paths, |_| true, |_| {}, &mut routes);

    let message = format!("{:#}", restarted.expect_err("copy 2's publish must fail"));
    assert!(
        message.contains("changed: myapp@web."),
        "the error must name the web copy now on the new env: {message}"
    );
    let live_env = fs::read_to_string(paths.env_root.join("myapp").join("LIVE_ENV"))
        .expect("LIVE_ENV must be written once one web copy runs the new env");
    assert!(
        live_env.lines().any(|line| line == "FLAG=new"),
        "LIVE_ENV must match the rolled copy: {live_env}"
    );
    drop(mock_supervisor);
}

/// The web kind rolls onto the new env, then writing the worker's config
/// fails. The app must not be left with web on the new env behind an old
/// `LIVE_ENV` and no reload: `LIVE_ENV` follows web, the supervisor is told
/// to reload, and the error names the worker as left behind.
#[test]
fn a_failed_worker_write_after_web_rolled_keeps_live_env_and_supervisor_in_step() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    let worker = deployed_app(&paths);
    let app_path = paths.app_root.join("myapp");
    let mock_supervisor = MockSupervisor::start(&paths);
    let mut routes = MockRoutes::new(&paths);
    let processes = HashMap::from([
        ("web".to_string(), "python app.py".to_string()),
        ("worker".to_string(), "python jobs.py".to_string()),
    ]);
    let old = HashMap::from([("FLAG".to_string(), "old".to_string())]);
    workers::create_workers_with(
        "myapp",
        &app_path,
        Some(&processes),
        &old,
        &paths,
        None,
        &mut routes,
    )
    .unwrap();
    fs::write(paths.env_root.join("myapp").join("ENV"), "FLAG=new\n").unwrap();
    // A directory where the worker's config goes makes its write fail even
    // for root, and only once the web kind has started rolling.
    routes.after_publish = Some(Box::new(move || {
        if worker.is_file() {
            fs::remove_file(&worker).unwrap();
            fs::create_dir(&worker).unwrap();
        }
    }));
    let reloads = Cell::new(0);

    let restarted = restart_procfile_app_with(
        "myapp",
        None,
        &paths,
        |_| true,
        |_| reloads.set(reloads.get() + 1),
        &mut routes,
    );

    let message = format!("{:#}", restarted.expect_err("the worker write must fail"));
    assert!(
        message.contains("old env: worker"),
        "the error must name the worker as left on the old env: {message}"
    );
    assert_eq!(enabled_web_flag(&paths).as_deref(), Some("new"));
    let live_env = fs::read_to_string(paths.env_root.join("myapp").join("LIVE_ENV"))
        .expect("LIVE_ENV must be written for the web kind now on the new env");
    assert!(
        live_env.lines().any(|line| line == "FLAG=new"),
        "LIVE_ENV must match the web env: {live_env}"
    );
    assert_eq!(reloads.get(), 1, "the supervisor must be told to reload");
    drop(mock_supervisor);
}
