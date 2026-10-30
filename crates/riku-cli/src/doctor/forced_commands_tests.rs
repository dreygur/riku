use super::*;
use crate::doctor::Status;

#[test]
fn warns_with_count_for_old_lines_and_passes_otherwise() {
    let old = r#"command="FINGERPRINT=SHA256:ab NAME=default /bin/riku $SSH_ORIGINAL_COMMAND",no-pty ssh-ed25519 AAAA"#;
    let warn = check_text(&format!("{old}\n{old}\n"));
    assert_eq!(warn.status, Status::Warn);
    assert!(warn.detail.contains("2 deploy keys"), "{}", warn.detail);
    assert_eq!(
        check_text("ssh-ed25519 AAAA admin@host\n").status,
        Status::Ok
    );
}

#[test]
#[cfg(unix)]
fn unreadable_authorized_keys_warns_instead_of_passing() {
    use std::os::unix::fs::PermissionsExt;
    if nix::unistd::geteuid().is_root() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("authorized_keys");
    std::fs::write(&path, "ssh-ed25519 AAAA x@y\n").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();
    let check = check_file(&path);
    assert_eq!(check.status, Status::Warn, "detail: {}", check.detail);
    assert!(check.detail.contains("authorized_keys"), "{}", check.detail);
}
