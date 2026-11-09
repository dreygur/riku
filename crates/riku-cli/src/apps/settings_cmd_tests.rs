use std::fs;
use std::os::unix::fs::PermissionsExt;

use super::*;

fn paths_with_app(tmp: &tempfile::TempDir) -> RikuPaths {
    let paths = RikuPaths::for_tests(tmp.path());
    fs::create_dir_all(paths.app_root.join("shop")).unwrap();
    paths
}

#[test]
fn set_then_get_round_trips_and_file_is_private() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths_with_app(&tmp);

    apply_set(
        &paths,
        "shop",
        &["routing.domains=a.example,b.example".to_string()],
    )
    .unwrap();

    let settings = load_override(&paths, "shop").unwrap();
    assert_eq!(
        settings.get("routing.domains").as_deref(),
        Some("a.example,b.example")
    );
    let mode = fs::metadata(paths.app_settings_override("shop"))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600, "override is {:o}", mode & 0o777);
}

#[test]
fn set_refuses_unknown_key_and_writes_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths_with_app(&tmp);

    assert!(apply_set(&paths, "shop", &["routing.domain=x".to_string()]).is_err());
    assert!(!paths.app_settings_override("shop").exists());
}

#[test]
fn unset_removes_key() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths_with_app(&tmp);
    apply_set(&paths, "shop", &["workers.timeout=60".to_string()]).unwrap();

    apply_unset(&paths, "shop", &["workers.timeout".to_string()]).unwrap();

    assert_eq!(
        load_override(&paths, "shop")
            .unwrap()
            .get("workers.timeout"),
        None
    );
}

#[test]
fn show_masks_secrets() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths_with_app(&tmp);
    apply_set(
        &paths,
        "shop",
        &["secrets.ghcr_token=topsecret".to_string()],
    )
    .unwrap();

    let lines = show_lines(&load_override(&paths, "shop").unwrap());

    assert_eq!(lines, vec!["secrets.ghcr_token = ***".to_string()]);
}

#[test]
fn config_set_refuses_a_riku_setting_and_keeps_env() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths_with_app(&tmp);
    let env_file = paths.env_root.join("shop").join("ENV");
    fs::create_dir_all(env_file.parent().unwrap()).unwrap();
    fs::write(&env_file, "PORT=5000\n").unwrap();

    let message = super::super::config::cmd_config_set(
        &paths,
        "shop",
        &["RIKU_DOMAINS=shop.example".to_string()],
    )
    .unwrap_err()
    .to_string();

    assert!(message.contains("routing.domains"), "{message}");
    assert_eq!(fs::read_to_string(&env_file).unwrap(), "PORT=5000\n");
}

#[test]
fn refuses_invalid_or_unknown_app_name() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths_with_app(&tmp);

    assert!(known_app(&paths, "../x").is_err());
    assert!(known_app(&paths, "missing").is_err());
    assert_eq!(known_app(&paths, "shop").unwrap(), "shop");
}
