use std::os::unix::fs::MetadataExt;

use nix::unistd::Uid;

use super::*;

#[test]
fn var_root_is_open_and_its_dirs_are_setgid_and_closed() {
    let tmp = tempfile::tempdir().unwrap();
    let var_root = tmp.path().join("riku");
    let acme = var_root.join("acme");
    let static_root = var_root.join("static");

    prepare_var_root(
        &var_root,
        Uid::current().as_raw(),
        None,
        &[&acme, &static_root],
    )
    .unwrap();

    let mode = |path: &Path| fs::metadata(path).unwrap().mode() & 0o7777;
    assert_eq!(mode(&var_root), 0o755, "var root mode");
    assert_eq!(mode(&acme), 0o2750, "ACME webroot mode");
    assert_eq!(mode(&static_root), 0o2750, "static root mode");
}

#[test]
fn refuses_a_parent_others_can_write() {
    let tmp = tempfile::tempdir().unwrap();
    let parent = tmp.path().join("var-lib");
    fs::create_dir(&parent).unwrap();
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o777)).unwrap();
    let var_root = parent.join("riku");

    let err = prepare_var_root(&var_root, Uid::current().as_raw(), None, &[])
        .unwrap_err()
        .to_string();

    assert!(err.contains("world writable"), "{err}");
    assert!(!var_root.exists(), "var root created anyway");
}
