use super::*;

fn paths() -> (tempfile::TempDir, RikuPaths) {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    std::fs::create_dir_all(&paths.riku_root).unwrap();
    (tmp, paths)
}

#[test]
fn missing_file_means_nginx() {
    let (_tmp, paths) = paths();

    assert_eq!(Router::load(&paths).unwrap(), Router::Nginx);
}

#[test]
fn stored_is_none_without_settings_or_file() {
    let (_tmp, paths) = paths();
    assert_eq!(stored(&paths).unwrap(), None);

    std::fs::write(&paths.router_file, "caddy\n").unwrap();
    assert_eq!(stored(&paths).unwrap(), Some(Router::Caddy));
}

#[test]
fn trims_and_folds_case() {
    let (_tmp, paths) = paths();

    std::fs::write(&paths.router_file, " Caddy \n").unwrap();
    assert_eq!(Router::load(&paths).unwrap(), Router::Caddy);

    std::fs::write(&paths.router_file, "traefik").unwrap();
    assert_eq!(
        Router::load(&paths).unwrap(),
        Router::Plugin("traefik".into())
    );
}

// A directory stands in for a permission error, which a root test runner
// cannot produce.
#[test]
fn unreadable_file_errors() {
    let (_tmp, paths) = paths();
    std::fs::create_dir(&paths.router_file).unwrap();

    let err = Router::load(&paths).unwrap_err().to_string();

    assert!(err.contains("ROUTER"), "error should name the file: {err}");
}

#[test]
fn router_from_settings_wins_over_legacy_file() {
    let (_tmp, paths) = paths();
    std::fs::write(&paths.router_file, "nginx\n").unwrap();
    std::fs::write(&paths.settings_file, "[router]\nname = \"caddy\"\n").unwrap();

    assert_eq!(Router::load(&paths).unwrap(), Router::Caddy);
}

#[test]
fn legacy_router_file_read_without_settings_key() {
    let (_tmp, paths) = paths();
    std::fs::write(&paths.router_file, "caddy\n").unwrap();
    std::fs::write(&paths.settings_file, "[engine]\ncall_timeout = \"30s\"\n").unwrap();

    assert_eq!(Router::load(&paths).unwrap(), Router::Caddy);
}

#[test]
fn invalid_settings_fails_router_load() {
    let (_tmp, paths) = paths();
    std::fs::write(&paths.router_file, "caddy\n").unwrap();
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
    assert!(
        !paths.router_file.exists(),
        "save must not write the legacy file"
    );
}

#[test]
fn save_then_load_round_trips() {
    let (_tmp, paths) = paths();

    save(&paths, &Router::Caddy).unwrap();

    assert_eq!(Router::load(&paths).unwrap(), Router::Caddy);
}
