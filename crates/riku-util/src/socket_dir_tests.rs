use std::os::unix::fs::{symlink, PermissionsExt};

use super::*;

const NO_SUCH_GROUP: &str = "riku-test-no-such-group";

#[test]
fn creates_a_directory_others_cannot_enter() {
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = tmp.path().join("sockets");

    ensure_exists(&dir, NO_SUCH_GROUP).unwrap();

    let mode = fs::metadata(&dir).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o750, "socket dir mode is {mode:o}");
}

#[test]
fn refuses_a_symlink_in_place_of_the_directory() {
    let tmp = tempfile::TempDir::new().unwrap();
    let target = tmp.path().join("elsewhere");
    fs::create_dir(&target).unwrap();
    fs::set_permissions(&target, fs::Permissions::from_mode(0o700)).unwrap();
    let dir = tmp.path().join("sockets");
    symlink(&target, &dir).unwrap();

    let err = prepare(&dir, None, NO_SUCH_GROUP).unwrap_err().to_string();

    assert!(err.contains("not a directory"), "{err}");
    let mode = fs::metadata(&target).unwrap().permissions().mode();
    assert_eq!(mode & 0o7777, 0o700, "the symlink target was chmodded");
}
