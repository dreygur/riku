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
fn a_static_site_with_no_runtime_says_static_files_need_one() {
    let tmp = TempDir::new().unwrap();
    let app_path = tmp.path().join("docs");
    fs::create_dir_all(app_path.join("public")).unwrap();
    fs::write(app_path.join("public").join("index.html"), "hi").unwrap();
    let env = HashMap::from([("RIKU_NGINX_STATIC".to_string(), "true".to_string())]);

    let err = detect_runtime("docs", &app_path, &paths_without_plugins(&tmp), &env).unwrap_err();

    assert!(
        err.to_string()
            .contains("riku serves static files only next to a runtime"),
        "got: {err}"
    );
}

fn enable_worker(paths: &RikuPaths, app: &str, kind: &str) {
    fs::create_dir_all(&paths.workers_enabled).unwrap();
    let config = crate::supervisor::config::create_worker_config(
        app,
        kind,
        "run",
        1,
        HashMap::new(),
        "/srv/app",
        "/srv/log",
    );
    let file = paths.workers_enabled.join(format!("{app}@{kind}.1.toml"));
    fs::write(file, toml::to_string(&config).unwrap()).unwrap();
}

fn router_name(paths: &RikuPaths) -> String {
    match Router::load(paths) {
        Ok(router) => router.name().to_string(),
        Err(_) => String::new(),
    }
}

#[test]
fn a_worker_only_app_prints_no_route_step() {
    let tmp = TempDir::new().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    enable_worker(&paths, "jobs", "worker");

    assert_eq!(route_step("jobs", &HashMap::new(), &paths), None);
}

#[test]
fn web_apps_and_static_sites_still_print_their_router() {
    let tmp = TempDir::new().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    enable_worker(&paths, "shop", "web");
    let static_env = HashMap::from([("RIKU_NGINX_STATIC".to_string(), "true".to_string())]);

    assert_eq!(
        route_step("shop", &HashMap::new(), &paths),
        Some(router_name(&paths)),
        "a web app keeps its Route step"
    );
    assert_eq!(
        route_step("docs", &static_env, &paths),
        Some(router_name(&paths)),
        "a static site keeps its Route step"
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
