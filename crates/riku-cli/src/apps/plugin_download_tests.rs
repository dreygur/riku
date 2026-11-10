use super::*;

const HELLO_SHA256: &str = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";

#[test]
fn binary_url_is_pinned_to_the_version() {
    let url = binary_url("java", "0.1.2", "x86_64-unknown-linux-gnu");

    assert_eq!(
        url,
        "https://github.com/dreygur/riku/releases/download/v0.1.2/riku-plugin-java-x86_64-unknown-linux-gnu"
    );
    assert!(!url.contains("latest"));
}

#[test]
fn checksum_file_first_field_is_the_hash() {
    let text = format!("{HELLO_SHA256}  riku-plugin-java-x86_64-unknown-linux-gnu\n");

    assert_eq!(expected_sha256(&text), Some(HELLO_SHA256));
    assert_eq!(expected_sha256("not a hash\n"), None);
}

#[test]
fn matching_bytes_pass() {
    assert!(verify("java", "0.1.2", b"hello", HELLO_SHA256).is_ok());
}

#[test]
fn checksum_mismatch_is_a_hard_error_naming_the_plugin() {
    let message = verify("java", "0.1.2", b"tampered", HELLO_SHA256)
        .unwrap_err()
        .to_string();

    assert!(
        message.contains("checksum mismatch for plugin 'java'") && message.contains("v0.1.2"),
        "{message}"
    );
}

#[test]
fn host_target_names_the_plugin_when_unsupported() {
    let message = target_triple_for("java", "freebsd", "x86_64")
        .unwrap_err()
        .to_string();

    assert!(
        message.contains("'java'") && message.contains("freebsd/x86_64"),
        "{message}"
    );
}
