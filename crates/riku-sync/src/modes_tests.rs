use super::*;

#[test]
fn symlink_in_place_of_a_directory_is_refused_not_followed() {
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("target");
    fs::create_dir(&target).unwrap();
    fs::set_permissions(&target, fs::Permissions::from_mode(0o755)).unwrap();
    let link = tmp.path().join("link");
    std::os::unix::fs::symlink(&target, &link).unwrap();

    let changed = chmod_dir(&link, |_| 0o700);

    assert!(
        changed.is_err(),
        "chmod went through the symlink: {changed:?}"
    );
    let mode = fs::metadata(&target).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o755, "the link target was changed");
}
