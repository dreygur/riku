use super::*;
use std::os::unix::fs::symlink;

#[test]
fn removes_a_file_and_a_link_without_following_the_link() {
    let tmp = tempfile::tempdir().unwrap();
    let bin = tmp.path().join(".local/bin");
    fs::create_dir_all(&bin).unwrap();
    let target = tmp.path().join("keep");
    fs::write(&target, "x").unwrap();

    fs::write(bin.join("riku"), "old").unwrap();
    remove_stale_binary(tmp.path()).unwrap();
    assert!(!bin.join("riku").exists());

    symlink(&target, bin.join("riku")).unwrap();
    remove_stale_binary(tmp.path()).unwrap();
    assert!(fs::symlink_metadata(bin.join("riku")).is_err());
    assert!(target.is_file(), "the link target must survive");
}

#[test]
fn leaves_a_directory_and_a_missing_path_alone() {
    let tmp = tempfile::tempdir().unwrap();
    remove_stale_binary(tmp.path()).unwrap();

    let dir = tmp.path().join(".local/bin/riku");
    fs::create_dir_all(&dir).unwrap();
    remove_stale_binary(tmp.path()).unwrap();
    assert!(dir.is_dir());
}
