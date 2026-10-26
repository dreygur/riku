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
fn save_then_load_round_trips() {
    let (_tmp, paths) = paths();

    save(&paths, &Router::Caddy).unwrap();

    assert_eq!(Router::load(&paths).unwrap(), Router::Caddy);
}
