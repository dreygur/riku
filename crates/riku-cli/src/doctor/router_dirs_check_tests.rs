use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};

use super::*;
use crate::doctor::Status;

fn dir_with_mode(tmp: &tempfile::TempDir, name: &str, mode: u32) -> PathBuf {
    let dir = tmp.path().join(name);
    fs::create_dir(&dir).unwrap();
    fs::set_permissions(&dir, fs::Permissions::from_mode(mode)).unwrap();
    dir
}

#[test]
fn a_root_dir_nobody_else_can_write_is_ok_with_owner_and_mode() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = dir_with_mode(&tmp, "nginx", 0o755);
    let me = fs::metadata(tmp.path()).unwrap().uid();

    let checks = router_dirs(std::slice::from_ref(&dir), me);

    assert_eq!(checks.len(), 1);
    assert_eq!(checks[0].status, Status::Ok, "{}", checks[0].detail);
    assert_eq!(
        checks[0].detail,
        format!("{}: owner uid {me}, mode 755", dir.display())
    );
}

#[test]
fn a_dir_the_group_can_write_fails() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = dir_with_mode(&tmp, "caddy", 0o2770);
    let me = fs::metadata(tmp.path()).unwrap().uid();

    let checks = router_dirs(&[dir], me);

    assert_eq!(checks[0].status, Status::Fail, "{}", checks[0].detail);
    assert!(
        checks[0].detail.contains("sudo riku init"),
        "{}",
        checks[0].detail
    );
}

#[test]
fn a_dir_another_user_owns_fails() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = dir_with_mode(&tmp, "certs", 0o700);
    let me = fs::metadata(tmp.path()).unwrap().uid();

    let checks = router_dirs(&[dir], me + 1);

    assert_eq!(checks[0].status, Status::Fail, "{}", checks[0].detail);
}

#[test]
fn a_symlink_in_place_of_a_dir_fails() {
    let tmp = tempfile::tempdir().unwrap();
    let target = dir_with_mode(&tmp, "target", 0o755);
    let link = tmp.path().join("nginx");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    let me = fs::metadata(tmp.path()).unwrap().uid();

    let checks = router_dirs(&[link], me);

    assert_eq!(checks[0].status, Status::Fail, "{}", checks[0].detail);
}

#[test]
fn missing_dirs_give_one_ok_line() {
    let tmp = tempfile::tempdir().unwrap();

    let checks = router_dirs(&[tmp.path().join("a"), tmp.path().join("b")], 0);

    assert_eq!(checks.len(), 1);
    assert_eq!(checks[0].status, Status::Ok, "{}", checks[0].detail);
}
