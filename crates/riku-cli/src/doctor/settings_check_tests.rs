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
fn missing_routing_section_is_reported() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    std::fs::create_dir_all(&paths.riku_root).unwrap();
    std::fs::write(&paths.settings_file, "[router]\nname = \"nginx\"\n").unwrap();

    let check = routing_section(&paths);

    assert_eq!(check.map(|c| c.status), Some(Status::Warn));
}

#[test]
fn present_routing_section_is_not_reported() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    std::fs::create_dir_all(&paths.riku_root).unwrap();
    std::fs::write(&paths.settings_file, "[routing]\nshared_port = 9000\n").unwrap();

    assert!(routing_section(&paths).is_none());
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
