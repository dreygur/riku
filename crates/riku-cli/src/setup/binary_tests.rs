use super::*;
use std::os::unix::fs::MetadataExt;

/// A running supervisor holds the installed binary open, and Linux refuses
/// writes into it (ETXTBSY). The old file must be left untouched: a hard
/// link to it still reads the old bytes after the replace.
#[test]
fn replaces_the_binary_without_writing_into_the_old_file() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("new-riku");
    let target = dir.path().join("riku");
    let running = dir.path().join("running-riku");
    fs::write(&source, "new").unwrap();
    fs::write(&target, "old").unwrap();
    fs::hard_link(&target, &running).unwrap();

    replace_binary(&source, &target).unwrap();

    assert_eq!(fs::read_to_string(&target).unwrap(), "new");
    assert_eq!(fs::read_to_string(&running).unwrap(), "old");
    let mode = fs::metadata(&target).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o755, "installed mode is {mode:o}");
}

/// Containers and routers are add-ons: the install needs only a directory
/// the installer owns, so a host with no nginx, Caddy or engine gets it too.
#[test]
fn install_runs_without_router() {
    let host = tempfile::tempdir().unwrap();
    let bin_dir = host.path().join("bin");
    fs::create_dir(&bin_dir).unwrap();
    fs::set_permissions(&bin_dir, fs::Permissions::from_mode(0o755)).unwrap();
    let owner = fs::metadata(&bin_dir).unwrap().uid();
    let source = host.path().join("riku-build");
    let target = bin_dir.join("riku");
    fs::write(&source, "binary").unwrap();

    install_at(&source, &target, owner).unwrap();

    assert_eq!(fs::read_to_string(&target).unwrap(), "binary");
    let mode = fs::metadata(&target).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o755, "installed mode is {mode:o}");

    fs::set_permissions(&bin_dir, fs::Permissions::from_mode(0o775)).unwrap();
    let err = install_at(&source, &target, owner).unwrap_err().to_string();
    assert!(err.contains("group or world writable"), "{err}");
}

#[test]
fn refuses_an_installed_binary_root_does_not_own_or_others_can_write() {
    let host = tempfile::tempdir().unwrap();
    let target = host.path().join("riku");
    fs::write(&target, "binary").unwrap();
    fs::set_permissions(&target, fs::Permissions::from_mode(0o755)).unwrap();
    let owner = fs::metadata(&target).unwrap().uid();
    assert!(
        install_at(&target, &target, owner).is_ok(),
        "a 0755 file of the owner"
    );

    let err = install_at(&target, &target, owner + 1)
        .unwrap_err()
        .to_string();
    assert!(err.contains("not root"), "{err}");

    fs::set_permissions(&target, fs::Permissions::from_mode(0o775)).unwrap();
    let err = install_at(&target, &target, owner).unwrap_err().to_string();
    assert!(err.contains("group or world writable"), "{err}");
}

#[test]
fn refuses_to_install_a_binary_the_deploy_user_owns() {
    let host = tempfile::tempdir().unwrap();
    let source = host.path().join("riku");
    fs::write(&source, "binary").unwrap();
    let uid = fs::metadata(&source).unwrap().uid();

    assert!(check_source_owner(&source, Some(uid + 1)).is_ok());
    assert!(check_source_owner(&source, None).is_ok());
    let err = check_source_owner(&source, Some(uid))
        .unwrap_err()
        .to_string();
    assert!(err.contains("owned by the deploy user"), "{err}");
}
