use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::ExitStatusExt;

use super::*;
use crate::fake_supervisor::FakeSupervisor;

/// A runtime plugin that takes any app, leaves `built` in the checkout when
/// its `build` runs, and adds `PLUGIN_ENV=1` from its `env`.
const PLUGIN: &str = r#"#!/bin/sh
case "$1" in
  detect) exit 0 ;;
  build)  touch "$RIKU_APP_PATH/built" ;;
  env)    echo "PLUGIN_ENV=1" ;;
  start)  ;;
esac
"#;

/// `myapp` deployed with one `worker` copy while its ENV said `FLAG=old`.
fn deployed_app(paths: &RikuPaths) -> PathBuf {
    for dir in [
        &paths.workers_available,
        &paths.workers_enabled,
        &paths.plugin_root,
    ] {
        fs::create_dir_all(dir).unwrap();
    }
    let plugin = paths.plugin_root.join("fake");
    fs::write(&plugin, PLUGIN).unwrap();
    fs::set_permissions(&plugin, fs::Permissions::from_mode(0o755)).unwrap();
    let app_path = paths.app_root.join("myapp");
    fs::create_dir_all(&app_path).unwrap();
    fs::create_dir_all(paths.env_root.join("myapp")).unwrap();
    fs::create_dir_all(paths.log_root.join("myapp")).unwrap();
    let processes = HashMap::from([("worker".to_string(), "python jobs.py".to_string())]);
    let env = HashMap::from([("FLAG".to_string(), "old".to_string())]);
    workers::create_workers_generic("myapp", &app_path, Some(&processes), &env, paths, None)
        .unwrap();
    paths.workers_available.join("myapp@worker.1.toml")
}

#[test]
fn restart_rewrites_configs_from_the_edited_env_without_a_build() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    let config = deployed_app(&paths);
    fs::write(paths.env_root.join("myapp").join("ENV"), "FLAG=new\n").unwrap();
    let mut supervisor = FakeSupervisor::start(&paths);

    let restarted = restart_procfile_app_with("myapp", None, &paths, |_| false);

    let status = supervisor.child.wait().unwrap();
    assert_eq!(restarted.ok(), Some(1), "one worker restarted");
    let rewritten: WorkerConfig = toml::from_str(&fs::read_to_string(&config).unwrap()).unwrap();
    assert_eq!(
        rewritten.env.get("FLAG").map(String::as_str),
        Some("new"),
        "the config must carry the value now in ENV"
    );
    assert_eq!(
        rewritten.env.get("PLUGIN_ENV").map(String::as_str),
        Some("1"),
        "the runtime plugin's env is added as on a deploy"
    );
    assert_eq!(rewritten.worker.command, "python jobs.py");
    assert!(
        !paths.app_root.join("myapp").join("built").exists(),
        "restart must not run the plugin's build"
    );
    assert_eq!(
        status.signal(),
        Some(nix::sys::signal::Signal::SIGHUP as i32),
        "the supervisor must get a SIGHUP to reload"
    );
}

#[test]
fn restart_is_refused_while_env_holds_the_removed_auto_restart_key() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    let config = deployed_app(&paths);
    let before = fs::read_to_string(&config).unwrap();
    fs::write(
        paths.env_root.join("myapp").join("ENV"),
        "RIKU_AUTO_RESTART=false\n",
    )
    .unwrap();
    let _supervisor = FakeSupervisor::start(&paths);

    let restarted = restart_procfile_app_with("myapp", None, &paths, |_| false);

    assert!(
        restarted.is_err(),
        "the removed key must refuse the restart"
    );
    assert_eq!(
        fs::read_to_string(&config).unwrap(),
        before,
        "config unchanged"
    );
}
