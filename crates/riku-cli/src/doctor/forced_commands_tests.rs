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
