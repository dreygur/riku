use super::*;
use std::fs::{self, File};
use tempfile::TempDir;

fn compose_arg(dir: &Path, requested: Option<&str>) -> Option<String> {
    find_compose_file(dir, requested)
        .expect("compose lookup should succeed")
        .map(|compose| compose.arg())
}

fn rejection(dir: &Path, requested: &str) -> String {
    match find_compose_file(dir, Some(requested)) {
        Ok(found) => panic!("COMPOSE_FILE='{requested}' should be rejected, got {found:?}"),
        Err(e) => e.to_string(),
    }
}

#[test]
fn compose_file_prefers_compose_yaml_over_docker_compose_yml() {
    let tmp = TempDir::new().unwrap();
    File::create(tmp.path().join("docker-compose.yml")).unwrap();
    File::create(tmp.path().join("compose.yaml")).unwrap();

    assert_eq!(
        compose_arg(tmp.path(), None).as_deref(),
        Some("compose.yaml")
    );
}

#[test]
fn compose_file_none_when_absent() {
    let tmp = TempDir::new().unwrap();
    assert_eq!(compose_arg(tmp.path(), None), None);
}

#[test]
fn compose_file_env_overrides_default_names_and_keeps_subdirectory() {
    let tmp = TempDir::new().unwrap();
    fs::create_dir(tmp.path().join("deploy")).unwrap();
    File::create(tmp.path().join("compose.yml")).unwrap();
    File::create(tmp.path().join("deploy/compose.prod.yml")).unwrap();

    assert_eq!(
        compose_arg(tmp.path(), Some("deploy/compose.prod.yml")).as_deref(),
        Some("deploy/compose.prod.yml")
    );
}

#[test]
fn compose_file_env_rejects_paths_outside_the_app() {
    let tmp = TempDir::new().unwrap();
    let app = tmp.path().join("app");
    fs::create_dir(&app).unwrap();
    File::create(tmp.path().join("outside.yml")).unwrap();

    for requested in ["../outside.yml", "/etc/hosts"] {
        let message = rejection(&app, requested);
        assert!(
            message.contains("relative path inside the app"),
            "unexpected error for {requested}: {message}"
        );
    }
}

#[test]
fn compose_file_env_rejects_symlink_escaping_the_app() {
    let tmp = TempDir::new().unwrap();
    let app = tmp.path().join("app");
    fs::create_dir(&app).unwrap();
    File::create(tmp.path().join("outside.yml")).unwrap();
    std::os::unix::fs::symlink(tmp.path().join("outside.yml"), app.join("compose.yml")).unwrap();

    let message = rejection(&app, "compose.yml");
    assert!(
        message.contains("must point to a file inside the app"),
        "unexpected error: {message}"
    );
}

#[test]
fn compose_file_env_rejects_missing_file_and_file_lists() {
    let tmp = TempDir::new().unwrap();
    File::create(tmp.path().join("compose.yml")).unwrap();

    let missing = rejection(tmp.path(), "compose.prod.yml");
    assert!(missing.contains("not found"), "unexpected error: {missing}");

    let list = rejection(tmp.path(), "compose.yml:compose.prod.yml");
    assert!(list.contains("exactly one"), "unexpected error: {list}");
}
