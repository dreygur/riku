use super::*;

fn paths() -> (tempfile::TempDir, RikuPaths) {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    std::fs::create_dir_all(&paths.riku_root).unwrap();
    (tmp, paths)
}

#[test]
fn missing_setting_means_nginx() {
    let (_tmp, paths) = paths();

    assert_eq!(stored(&paths).unwrap(), None);
    assert_eq!(Router::load(&paths).unwrap(), Router::Nginx);
}

#[test]
fn a_leftover_router_file_is_not_read() {
    let (_tmp, paths) = paths();
    std::fs::write(paths.riku_root.join("ROUTER"), "caddy\n").unwrap();

    assert_eq!(Router::load(&paths).unwrap(), Router::Nginx);
}

#[test]
fn trims_and_folds_case() {
    assert_eq!(Router::from(" Caddy \n"), Router::Caddy);
    assert_eq!(Router::from("traefik"), Router::Plugin("traefik".into()));
}

#[test]
fn invalid_settings_fails_router_load() {
    let (_tmp, paths) = paths();
    std::fs::write(&paths.settings_file, "[router]\nnmae = \"caddy\"\n").unwrap();

    let err = Router::load(&paths).unwrap_err().to_string();

    assert!(
        err.contains("settings.toml"),
        "error should name the file: {err}"
    );
}

#[test]
fn save_writes_settings_and_keeps_engine_section() {
    let (_tmp, paths) = paths();
    std::fs::write(&paths.settings_file, "[engine]\ncall_timeout = \"45s\"\n").unwrap();

    save(&paths, &Router::Caddy).unwrap();

    let settings = crate::settings::load(&paths).unwrap();
    assert_eq!(settings.router.as_deref(), Some("caddy"));
    assert_eq!(settings.engine.call_timeout.as_secs(), 45);
}

#[test]
fn save_then_load_round_trips() {
    let (_tmp, paths) = paths();

    save(&paths, &Router::Caddy).unwrap();

    assert_eq!(Router::load(&paths).unwrap(), Router::Caddy);
}
