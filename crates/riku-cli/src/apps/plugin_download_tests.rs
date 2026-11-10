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
fn armv6_riku_fetches_the_armv6_plugin() {
    assert_eq!(
        release_triple("java", "arm-unknown-linux-gnueabihf").ok(),
        Some("arm-unknown-linux-gnueabihf")
    );
}

#[test]
fn musl_riku_fetches_the_musl_plugin() {
    assert_eq!(
        release_triple("java", "aarch64-unknown-linux-musl").ok(),
        Some("aarch64-unknown-linux-musl")
    );
}

#[test]
fn release_triples_match_the_release_matrix() {
    let workflow = include_str!("../../../../.github/workflows/release.yml");
    let mut matrix: Vec<&str> = workflow
        .lines()
        .filter_map(|line| line.trim().strip_prefix("target: "))
        .filter(|target| !target.starts_with("${{"))
        .collect();
    let mut known = RELEASE_TRIPLES.to_vec();
    matrix.sort_unstable();
    known.sort_unstable();

    assert_eq!(matrix, known, "RELEASE_TRIPLES drifted from release.yml");
}

#[test]
fn unshipped_target_names_plugin_and_triple() {
    let message = release_triple("java", "riscv64gc-unknown-linux-gnu")
        .unwrap_err()
        .to_string();

    assert!(
        message.contains("'java'") && message.contains("riscv64gc-unknown-linux-gnu"),
        "{message}"
    );
}

#[test]
fn a_body_over_the_limit_is_refused() {
    let at_limit = read_capped(std::io::Cursor::new(vec![7u8; 8]), 8, "u");
    assert_eq!(at_limit.ok(), Some(vec![7u8; 8]));

    let Err(FetchError::Failed(reason)) = read_capped(std::io::Cursor::new(vec![7u8; 9]), 8, "u")
    else {
        panic!("a 9 byte body under an 8 byte limit should fail");
    };
    assert!(reason.contains("more than 8 bytes"), "{reason}");
}
