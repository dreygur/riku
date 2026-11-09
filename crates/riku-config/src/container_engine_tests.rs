use super::*;

fn paths() -> (tempfile::TempDir, RikuPaths) {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    std::fs::create_dir_all(&paths.riku_root).unwrap();
    (tmp, paths)
}

#[test]
fn missing_engine_file_means_detect() {
    let (_tmp, paths) = paths();

    assert_eq!(load(&paths).unwrap(), None);
}

#[test]
fn engine_file_is_trimmed_and_blank_means_detect() {
    let (_tmp, paths) = paths();

    std::fs::write(&paths.container_engine_file, "  podman\n").unwrap();
    assert_eq!(load(&paths).unwrap().as_deref(), Some("podman"));

    std::fs::write(&paths.container_engine_file, " \n").unwrap();
    assert_eq!(load(&paths).unwrap(), None);
}

#[test]
fn engine_program_from_settings_wins_over_legacy_file() {
    let (_tmp, paths) = paths();
    std::fs::write(&paths.container_engine_file, "docker\n").unwrap();
    std::fs::write(&paths.settings_file, "[engine]\nprogram = \"podman\"\n").unwrap();

    assert_eq!(load(&paths).unwrap().as_deref(), Some("podman"));
}

#[test]
fn unreadable_engine_file_is_an_error() {
    let (_tmp, paths) = paths();
    std::fs::create_dir(&paths.container_engine_file).unwrap();

    let err = load(&paths).unwrap_err().to_string();

    assert!(err.contains(ENGINE_FILE), "{err}");
}
