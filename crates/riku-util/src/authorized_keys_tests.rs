use super::*;
use std::fs;
use tempfile::TempDir;

const OLD: &str = r#"command="FINGERPRINT=SHA256:ab NAME=default /home/riku/.local/bin/riku $SSH_ORIGINAL_COMMAND",no-agent-forwarding,no-user-rc,no-X11-forwarding,no-port-forwarding ssh-ed25519 AAAA me@laptop"#;
const NEW: &str = r#"command="FINGERPRINT=SHA256:ab NAME=default /home/riku/.local/bin/riku",no-agent-forwarding,no-user-rc,no-X11-forwarding,no-port-forwarding ssh-ed25519 AAAA me@laptop"#;
const FOREIGN: &str = "ssh-ed25519 AAAA admin@host";

#[test]
fn rewrites_old_forced_command() {
    assert_eq!(migrate_line(OLD).as_deref(), Some(NEW));
}

#[test]
fn leaves_new_and_foreign_lines_alone() {
    assert_eq!(migrate_line(NEW), None);
    assert_eq!(migrate_line(FOREIGN), None);
    assert_eq!(
        migrate_line(r#"command="riku agent --scope staging",no-port-forwarding ssh-rsa AAAA"#),
        None
    );
    let unmarked = r#"command="/usr/bin/other $SSH_ORIGINAL_COMMAND",no-pty ssh-rsa AAAA"#;
    assert_eq!(migrate_line(unmarked), None);
}

#[test]
fn migrate_text_preserves_every_other_byte() {
    let text = format!("# comment\n\n{OLD}\r\n{FOREIGN}\n{OLD}");
    let (out, count) = migrate_text(&text);
    assert_eq!(count, 2);
    assert_eq!(out, format!("# comment\n\n{NEW}\r\n{FOREIGN}\n{NEW}"));
}

#[test]
fn migrate_file_keeps_mode_600_and_counts() {
    use std::os::unix::fs::PermissionsExt;
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("authorized_keys");
    fs::write(&path, format!("{OLD}\n{FOREIGN}\n")).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();

    assert_eq!(migrate_file(&path).unwrap(), 1);
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        format!("{NEW}\n{FOREIGN}\n")
    );
    let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600, "authorized_keys must stay 600");
    assert_eq!(migrate_file(&path).unwrap(), 0, "second run finds nothing");
}

#[test]
fn migrate_file_without_old_lines_does_not_touch_the_file() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("authorized_keys");
    fs::write(&path, FOREIGN).unwrap();
    assert_eq!(migrate_file(&path).unwrap(), 0);
    assert_eq!(fs::read_to_string(&path).unwrap(), FOREIGN);
    assert_eq!(
        fs::read_dir(dir.path()).unwrap().count(),
        1,
        "no temp file left behind"
    );
}

#[test]
fn migrate_file_missing_is_zero() {
    let dir = TempDir::new().unwrap();
    assert_eq!(migrate_file(&dir.path().join("nope")).unwrap(), 0);
}
