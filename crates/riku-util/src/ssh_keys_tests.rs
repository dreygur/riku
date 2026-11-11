use super::*;
use std::fs;
use tempfile::TempDir;

/// Runs `f` with a fresh home directory. The home is passed to
/// `setup_authorized_keys_in`, never set as `HOME`, so tests stay hermetic.
fn with_home<F: FnOnce(&TempDir)>(f: F) {
    let tmp = TempDir::new().unwrap();
    f(&tmp);
}

#[test]
fn test_setup_creates_ssh_dir_and_authorized_keys() {
    with_home(|tmp| {
        let result = setup_authorized_keys_in(
            tmp.path(),
            "fingerprint123",
            "/usr/bin/riku",
            "ssh-rsa AAAA user@host",
        );
        assert!(result.is_ok(), "{result:?}");
        assert!(tmp.path().join(".ssh").exists());
        assert!(tmp.path().join(".ssh/authorized_keys").exists());
    });
}

#[test]
fn test_setup_appends_correct_line_format() {
    with_home(|tmp| {
        setup_authorized_keys_in(
            tmp.path(),
            "fp42",
            "/usr/local/bin/riku",
            "ssh-rsa BBBB test@local",
        )
        .unwrap();
        let content = fs::read_to_string(tmp.path().join(".ssh/authorized_keys")).unwrap();
        assert!(content.contains("FINGERPRINT=fp42"));
        assert!(content.contains("/usr/local/bin/riku"));
        assert!(content.contains("ssh-rsa BBBB test@local"));
        assert!(content.contains("no-agent-forwarding"));
        assert!(content.contains("no-port-forwarding"));
    });
}

#[test]
fn test_setup_appends_multiple_keys() {
    with_home(|tmp| {
        setup_authorized_keys_in(tmp.path(), "fp1", "/usr/bin/riku", "ssh-rsa KEY1 a@b").unwrap();
        setup_authorized_keys_in(tmp.path(), "fp2", "/usr/bin/riku", "ssh-rsa KEY2 c@d").unwrap();
        let content = fs::read_to_string(tmp.path().join(".ssh/authorized_keys")).unwrap();
        assert!(content.contains("KEY1"));
        assert!(content.contains("KEY2"));
        assert_eq!(content.lines().count(), 2);
    });
}

#[test]
fn new_keys_get_plain_forced_command() {
    with_home(|tmp| {
        setup_authorized_keys_in(tmp.path(), "fp", "/usr/bin/riku", "ssh-rsa KEY user@host")
            .unwrap();
        let content = fs::read_to_string(tmp.path().join(".ssh/authorized_keys")).unwrap();
        assert!(
            !content.contains("$SSH_ORIGINAL_COMMAND"),
            "forced command must not expand the shell variable: {content}"
        );
        assert!(content.contains(r#"command="FINGERPRINT=fp NAME=default /usr/bin/riku","#));
    });
}

#[test]
#[cfg(unix)]
fn test_setup_sets_correct_permissions() {
    use std::os::unix::fs::PermissionsExt;
    with_home(|tmp| {
        setup_authorized_keys_in(tmp.path(), "fp", "/usr/bin/riku", "ssh-rsa KEY user@host")
            .unwrap();
        let ssh_dir = tmp.path().join(".ssh");
        let dir_mode = fs::metadata(&ssh_dir).unwrap().permissions().mode() & 0o777;
        assert_eq!(dir_mode, 0o700, "ssh dir should be 700");
        let file_mode = fs::metadata(ssh_dir.join("authorized_keys"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(file_mode, 0o600, "authorized_keys should be 600");
    });
}
