use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};

use super::*;

fn mode(path: &Path) -> u32 {
    fs::symlink_metadata(path).unwrap().mode() & 0o7777
}

#[test]
fn cert_root_and_app_dirs_get_the_mode_back() {
    let tmp = tempfile::tempdir().unwrap();
    let certs = tmp.path().join("certs");
    let app = certs.join("shop");
    fs::create_dir_all(&app).unwrap();
    for dir in [&certs, &app] {
        fs::set_permissions(dir, fs::Permissions::from_mode(0o777)).unwrap();
    }
    let me = fs::metadata(tmp.path()).unwrap().uid();

    secure_cert_dirs(&certs, me, None, 0o700).unwrap();

    assert_eq!((mode(&certs), mode(&app)), (0o700, 0o700));
}

#[test]
fn a_missing_cert_root_is_left_for_the_helper() {
    let tmp = tempfile::tempdir().unwrap();
    let me = fs::metadata(tmp.path()).unwrap().uid();

    secure_cert_dirs(&tmp.path().join("certs"), me, None, 0o700).unwrap();

    assert!(!tmp.path().join("certs").exists());
}

#[test]
fn a_symlink_in_place_of_a_dir_is_refused_and_its_target_untouched() {
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("target");
    fs::create_dir(&target).unwrap();
    fs::set_permissions(&target, fs::Permissions::from_mode(0o755)).unwrap();
    let link = tmp.path().join("certs");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    let me = fs::metadata(tmp.path()).unwrap().uid();

    let err = secure_cert_dirs(&link, me, None, 0o700)
        .unwrap_err()
        .to_string();

    assert!(err.contains("not a directory"), "{err}");
    assert_eq!(mode(&target), 0o755);
}
