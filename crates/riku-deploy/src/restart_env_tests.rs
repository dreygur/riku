use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;

use super::worker_env;
use crate::config::RikuPaths;
use crate::plugin_env;

/// A runtime plugin that takes any app and answers `env` with `FROM=plugin`,
/// leaving a `called` marker beside itself on every run.
const PLUGIN: &str = r#"#!/bin/sh
touch "$(dirname "$0")/called"
case "$1" in
  detect) exit 0 ;;
  env)    echo "FROM=plugin" ;;
esac
"#;

fn app_with_plugin(paths: &RikuPaths) {
    fs::create_dir_all(&paths.plugin_root).unwrap();
    let plugin = paths.plugin_root.join("fake");
    fs::write(&plugin, PLUGIN).unwrap();
    fs::set_permissions(&plugin, fs::Permissions::from_mode(0o755)).unwrap();
    fs::create_dir_all(paths.app_root.join("myapp")).unwrap();
    fs::create_dir_all(paths.env_root.join("myapp")).unwrap();
}

#[test]
fn restart_uses_the_stored_plugin_env_without_running_the_plugin() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    app_with_plugin(&paths);
    let stored = HashMap::from([("FROM".to_string(), "deploy".to_string())]);
    plugin_env::save("myapp", &paths, &stored).unwrap();

    let env = worker_env("myapp", &paths).unwrap();

    assert_eq!(env.get("FROM").map(String::as_str), Some("deploy"));
    assert!(
        !paths.plugin_root.join("called").exists(),
        "the runtime plugin must not run when the deploy stored its env"
    );
    let mode = fs::metadata(paths.env_root.join("myapp").join("PLUGIN_ENV"))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600, "PLUGIN_ENV may hold secrets");
}

#[test]
fn an_app_without_a_stored_plugin_env_runs_the_plugin() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    app_with_plugin(&paths);

    let env = worker_env("myapp", &paths).unwrap();

    assert_eq!(env.get("FROM").map(String::as_str), Some("plugin"));
}
