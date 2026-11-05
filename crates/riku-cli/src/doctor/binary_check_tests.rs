use super::*;
use crate::doctor::Status;
use std::os::unix::fs::PermissionsExt;

#[test]
fn flags_a_launch_line_naming_another_path() {
    let keys = "command=\"FINGERPRINT=SHA256:ab NAME=default /home/deploy/.local/bin/riku\",no-pty ssh-ed25519 AAAA me@laptop\n\
                command=\"FINGERPRINT=SHA256:cd NAME=default /usr/local/bin/riku\",no-pty ssh-ed25519 BBBB you@laptop\n";
    let unit = "ExecStart=/home/deploy/.local/bin/riku supervisor\nReadWritePaths=/home/riku\n";

    assert_eq!(stray_binaries(keys), ["/home/deploy/.local/bin/riku"]);
    assert_eq!(stray_binaries(unit), ["/home/deploy/.local/bin/riku"]);
    assert!(stray_binaries("ExecStart=/usr/local/bin/riku supervisor\n").is_empty());
}

#[test]
fn refuses_a_binary_the_deploy_user_can_write() {
    let dir = tempfile::tempdir().unwrap();
    let binary = dir.path().join("riku");
    fs::write(&binary, "binary").unwrap();
    let owner = fs::metadata(&binary).unwrap().uid();

    fs::set_permissions(&binary, fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(installed(&binary, owner).status, Status::Ok);

    fs::set_permissions(&binary, fs::Permissions::from_mode(0o775)).unwrap();
    let check = installed(&binary, owner);
    assert_eq!(check.status, Status::Fail, "{}", check.detail);
    assert_eq!(installed(&binary, owner + 1).status, Status::Fail);

    fs::set_permissions(&binary, fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(
        installed(&binary, owner).status,
        Status::Fail,
        "no exec bit"
    );
    assert_eq!(
        installed(&dir.path().join("missing"), owner).status,
        Status::Fail
    );
}
