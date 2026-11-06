use super::*;
use crate::doctor::Status;

#[test]
fn absent_file_is_ok_with_defaults() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());

    let check = settings_file(&paths);

    assert_eq!(check.status, Status::Ok, "{}", check.detail);
    assert_eq!(check.detail, "defaults");
}

#[test]
fn invalid_file_fails_naming_the_key() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    std::fs::create_dir_all(&paths.riku_root).unwrap();
    std::fs::write(&paths.settings_file, "[engine]\npull_timeout = \"soon\"\n").unwrap();

    let check = settings_file(&paths);

    assert_eq!(check.status, Status::Fail);
    assert!(
        check.detail.contains("pull_timeout"),
        "detail should name the key: {}",
        check.detail
    );
}
