use super::*;
use std::fs;
use tempfile::TempDir;

fn paths_without_plugins(tmp: &TempDir) -> RikuPaths {
    RikuPaths::from_dirs(tmp.path().join(".riku"), tmp.path())
}

#[test]
fn missing_bundled_plugin_names_the_install_command() {
    let tmp = TempDir::new().unwrap();
    let app_path = tmp.path().join("myapp");
    fs::create_dir(&app_path).unwrap();
    fs::write(app_path.join("package.json"), "{}").unwrap();

    let err = detect_runtime(
        "myapp",
        &app_path,
        &paths_without_plugins(&tmp),
        &HashMap::new(),
    )
    .unwrap_err();
    assert_eq!(
        err.to_string(),
        "app 'myapp' needs the node plugin. Install it with: rk plugins install node \
         (on the server: riku plugins install node), then push again."
    );

    let mut env = HashMap::new();
    env.insert("RIKU_RUNTIME".to_string(), "java".to_string());
    let err = detect_runtime("myapp", &app_path, &paths_without_plugins(&tmp), &env).unwrap_err();
    assert!(
        err.to_string().contains("rk plugins install java"),
        "RIKU_RUNTIME naming a shipped runtime should name its install command, got: {err}"
    );
}

#[test]
fn missing_custom_plugin_keeps_the_old_message() {
    let tmp = TempDir::new().unwrap();
    let app_path = tmp.path().join("myapp");
    fs::create_dir(&app_path).unwrap();
    let mut env = HashMap::new();
    env.insert("RIKU_RUNTIME".to_string(), "elixir".to_string());

    let err = detect_runtime("myapp", &app_path, &paths_without_plugins(&tmp), &env).unwrap_err();
    assert!(
        err.to_string()
            .starts_with("RIKU_RUNTIME='elixir' is set but no plugin"),
        "a custom plugin name should keep the old message, got: {err}"
    );
}
