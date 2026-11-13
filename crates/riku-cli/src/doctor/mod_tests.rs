use super::report::tally;
use super::*;
use crate::config::RikuPaths;

#[test]
fn tally_counts_each_status() {
    let checks = vec![
        Check::ok("a", ""),
        Check::ok("b", ""),
        Check::warn("c", ""),
        Check::fail("d", ""),
    ];
    assert_eq!(tally(&checks), (2, 1, 1));
}

#[test]
fn directories_fails_when_tree_missing() {
    let tmp = tempfile::tempdir().unwrap();
    // Root exists but the sub-directories do not.
    let paths = RikuPaths::from_dirs(tmp.path().to_path_buf(), tmp.path());
    let check = checks::directories(&paths);
    assert_eq!(check.status, Status::Fail);
    assert!(check.detail.contains("apps"));
}

#[test]
fn directories_ok_when_tree_present() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::from_dirs(tmp.path().to_path_buf(), tmp.path());
    for dir in [
        &paths.app_root,
        &paths.data_root,
        &paths.env_root,
        &paths.git_root,
        &paths.log_root,
        &paths.nginx_root,
        &paths.plugin_root,
        &paths.workers_enabled,
    ] {
        std::fs::create_dir_all(dir).unwrap();
    }
    let check = checks::directories(&paths);
    assert_eq!(check.status, Status::Ok);
}

#[test]
fn plugins_warns_when_empty() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::from_dirs(tmp.path().to_path_buf(), tmp.path());
    std::fs::create_dir_all(&paths.plugin_root).unwrap();
    std::fs::write(paths.plugin_root.join(".bundled"), "").unwrap();
    assert_eq!(
        checks::plugins(&paths).status,
        Status::Warn,
        "the hidden bundled record counted as a plugin"
    );
}

#[test]
fn plugins_ok_when_populated() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::from_dirs(tmp.path().to_path_buf(), tmp.path());
    std::fs::create_dir_all(&paths.plugin_root).unwrap();
    std::fs::write(paths.plugin_root.join("node"), "#!/bin/sh\n").unwrap();
    let check = checks::plugins(&paths);
    assert_eq!(check.status, Status::Ok);
    assert!(check.detail.contains('1'));
}

#[test]
fn disk_reports_free_space_for_existing_root() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::from_dirs(tmp.path().to_path_buf(), tmp.path());
    // A tempdir on a normal dev/CI box has well over 100 MiB free.
    assert_ne!(checks::disk(&paths).status, Status::Fail);
}

#[test]
fn check_constructors_set_status() {
    assert_eq!(Check::ok("n", "d").status, Status::Ok);
    assert_eq!(Check::warn("n", "d").status, Status::Warn);
    assert_eq!(Check::fail("n", "d").status, Status::Fail);
    // Detail is preserved verbatim.
    assert_eq!(Check::ok("n", "hello").detail, "hello".to_string());
}
