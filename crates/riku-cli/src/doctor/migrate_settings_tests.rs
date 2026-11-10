use std::fs;
use std::os::unix::fs::PermissionsExt;

use super::*;
use crate::config::app_settings::{AppSettings, Source};
use crate::doctor::Status;

fn paths(tmp: &tempfile::TempDir) -> RikuPaths {
    let paths = RikuPaths::for_tests(tmp.path());
    fs::create_dir_all(&paths.riku_root).unwrap();
    fs::create_dir_all(paths.env_root.join("shop")).unwrap();
    paths
}

fn env_file(paths: &RikuPaths) -> std::path::PathBuf {
    paths.env_root.join("shop").join("ENV")
}

fn write_env(paths: &RikuPaths, text: &str) {
    fs::write(env_file(paths), text).unwrap();
    fs::set_permissions(env_file(paths), fs::Permissions::from_mode(0o600)).unwrap();
}

#[test]
fn moves_router_file_into_settings() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths(&tmp);
    fs::write(&paths.router_file, "caddy\n").unwrap();

    migrate(&paths);

    assert!(!paths.router_file.exists(), "ROUTER should be removed");
    let settings = crate::config::settings::load(&paths).unwrap();
    assert_eq!(settings.router.as_deref(), Some("caddy"));
}

#[test]
fn moves_table_keys_out_of_env_keeping_other_lines() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths(&tmp);
    write_env(
        &paths,
        "# database\nDATABASE_URL=postgres://db\nRIKU_DOMAINS=shop.example\nPORT = 5000\n",
    );

    migrate(&paths);

    assert_eq!(
        fs::read_to_string(env_file(&paths)).unwrap(),
        "# database\nDATABASE_URL=postgres://db\nPORT = 5000\n"
    );
    let moved = AppSettings::load(&paths.app_settings_override("shop"), Source::Server)
        .unwrap()
        .unwrap();
    assert_eq!(
        moved.get("routing.domains").as_deref(),
        Some("shop.example")
    );
    assert!(paths.env_root.join("shop").join("ENV.bak").exists());
}

#[test]
fn keeps_lossy_keys_in_env_with_warning() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths(&tmp);
    write_env(&paths, "RIKU_NGINX_STATIC=false\nRIKU_HTTPS_ONLY=1\n");

    let checks = migrate(&paths);

    assert_eq!(
        fs::read_to_string(env_file(&paths)).unwrap(),
        "RIKU_NGINX_STATIC=false\nRIKU_HTTPS_ONLY=1\n"
    );
    for key in ["RIKU_NGINX_STATIC", "RIKU_HTTPS_ONLY"] {
        assert!(
            checks
                .iter()
                .any(|c| c.status == Status::Warn && c.detail.contains(key)),
            "no warning for {key}: {checks:?}"
        );
    }
}

#[test]
fn keeps_env_file_mode() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths(&tmp);
    write_env(&paths, "RIKU_DOMAINS=shop.example\nSECRET=x\n");

    migrate(&paths);

    let mode = fs::metadata(env_file(&paths)).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600, "ENV is {mode:o}");
}

#[test]
fn second_run_changes_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths(&tmp);
    fs::write(&paths.router_file, "caddy\n").unwrap();
    write_env(&paths, "RIKU_DOMAINS=shop.example\nPORT=5000\n");
    migrate(&paths);
    let env_after_first = fs::read_to_string(env_file(&paths)).unwrap();

    let checks = migrate(&paths);

    assert_eq!(
        fs::read_to_string(env_file(&paths)).unwrap(),
        env_after_first
    );
    assert_eq!(checks.len(), 1, "checks: {checks:?}");
    assert_eq!(checks[0].status, Status::Ok);
}

#[test]
fn adds_routing_section_once() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths(&tmp);
    fs::write(&paths.settings_file, "[router]\nname = \"nginx\"\n").unwrap();

    let first = migrate(&paths);
    let after_first = fs::read_to_string(&paths.settings_file).unwrap();
    migrate(&paths);

    assert!(
        first.iter().any(|check| check.detail.contains("[routing]")),
        "first run should report the added section: {first:?}"
    );
    assert!(
        crate::config::settings::load(&paths)
            .unwrap()
            .routing
            .present
    );
    assert_eq!(
        fs::read_to_string(&paths.settings_file).unwrap(),
        after_first
    );
}

#[test]
fn leaves_env_alone_when_another_line_expands_a_moved_key() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths(&tmp);
    fs::create_dir_all(paths.app_root.join("shop")).unwrap();
    let text = "RIKU_DOMAINS=shop.example\nAPP_URL=https://$RIKU_DOMAINS\n";
    write_env(&paths, text);

    let checks = migrate(&paths);

    assert_eq!(fs::read_to_string(env_file(&paths)).unwrap(), text);
    assert!(!paths.app_settings_override("shop").exists());
    assert!(
        checks
            .iter()
            .any(|c| c.status == Status::Warn && c.detail.contains("APP_URL")),
        "checks: {checks:?}"
    );
}

#[test]
fn leaves_env_alone_when_the_override_names_another_runtime() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths(&tmp);
    fs::create_dir_all(paths.app_root.join("shop")).unwrap();
    fs::write(
        paths.app_settings_override("shop"),
        "[runtime]\nname = \"python\"\n",
    )
    .unwrap();
    let text = "RIKU_RUNTIME=node\nRIKU_NODE_VERSION=20\n";
    write_env(&paths, text);
    let before = crate::config::app_config::load_env(&paths, "shop")
        .unwrap()
        .env;

    migrate(&paths);

    let after = crate::config::app_config::load_env(&paths, "shop")
        .unwrap()
        .env;
    assert_eq!(after, before);
}

#[test]
fn app_env_unchanged_after_migration() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths(&tmp);
    fs::create_dir_all(paths.app_root.join("shop")).unwrap();
    write_env(
        &paths,
        "RIKU_DOMAINS=shop.example\nRIKU_WORKER_TIMEOUT=60\nRIKU_RUNTIME=node\nRIKU_NODE_VERSION=22\nPORT=5000\n",
    );
    let before = crate::config::app_config::load_env(&paths, "shop")
        .unwrap()
        .env;

    migrate(&paths);

    let after = crate::config::app_config::load_env(&paths, "shop").unwrap();
    assert_eq!(after.env, before);
    assert!(
        after.legacy_keys.is_empty(),
        "left: {:?}",
        after.legacy_keys
    );
}
