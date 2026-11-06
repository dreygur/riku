use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use super::{find_in, find_program};

#[test]
fn find_in_skips_non_executable_and_picks_executable_in_later_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let dir1 = tmp.path().join("bin1");
    let dir2 = tmp.path().join("bin2");
    fs::create_dir_all(&dir1).unwrap();
    fs::create_dir_all(&dir2).unwrap();

    let non_exec = dir1.join("prog");
    fs::write(&non_exec, "#!/bin/sh\n").unwrap();
    fs::set_permissions(&non_exec, fs::Permissions::from_mode(0o644)).unwrap();

    let exec = dir2.join("prog");
    fs::write(&exec, "#!/bin/sh\n").unwrap();
    fs::set_permissions(&exec, fs::Permissions::from_mode(0o755)).unwrap();

    let found = find_in("prog", vec![dir1, dir2]);
    assert_eq!(found, Some(exec));
}

#[test]
fn find_in_returns_none_when_absent() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("bin");
    fs::create_dir_all(&dir).unwrap();

    let found = find_in("missing-program", vec![dir]);
    assert_eq!(found, None);
}

#[test]
fn find_program_returns_bare_name_when_absent() {
    let result = find_program("definitely-not-a-real-program-xyz");
    assert_eq!(result, PathBuf::from("definitely-not-a-real-program-xyz"));
}
