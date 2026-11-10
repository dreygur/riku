use super::*;
use crate::doctor::Status;

fn paths_with_env(app: &str, contents: &str) -> (tempfile::TempDir, RikuPaths) {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::from_dirs(tmp.path().to_path_buf(), tmp.path());
    let app_dir = paths.env_root.join(app);
    std::fs::create_dir_all(&app_dir).unwrap();
    std::fs::write(app_dir.join("ENV"), contents).unwrap();
    (tmp, paths)
}

#[test]
fn warns_on_renamed_key() {
    let (_tmp, paths) = paths_with_env("shop", "NGINX_SERVER_NAME=shop.example\nPORT=5000\n");

    let checks = renamed_keys(&paths);

    assert_eq!(checks.len(), 1, "expected one warning");
    assert_eq!(checks[0].status, Status::Warn);
    assert_eq!(
        checks[0].detail,
        "shop: NGINX_SERVER_NAME is no longer read, use RIKU_DOMAINS"
    );
}

#[test]
fn dropped_key_is_reported_as_one_to_remove() {
    let (_tmp, paths) = paths_with_env("shop", "NGINX_CACHE_PATH=/var/cache\n");

    let checks = renamed_keys(&paths);

    assert_eq!(
        checks[0].detail,
        "shop: NGINX_CACHE_PATH is no longer supported; remove it from ENV"
    );
}

#[test]
fn ok_with_only_app_keys() {
    let (_tmp, paths) = paths_with_env("shop", "PORT=5000\nDATABASE_URL=postgres://db\n");

    let checks = renamed_keys(&paths);

    assert_eq!(checks.len(), 1);
    assert_eq!(checks[0].status, Status::Ok, "detail: {}", checks[0].detail);
}

#[test]
fn warns_on_riku_setting_left_in_env() {
    let (_tmp, paths) = paths_with_env("shop", "RIKU_DOMAINS=shop.example\n");

    let checks = renamed_keys(&paths);

    assert_eq!(checks.len(), 1);
    assert_eq!(checks[0].status, Status::Warn);
    assert!(
        checks[0].detail.contains("riku doctor --fix"),
        "{}",
        checks[0].detail
    );
}
