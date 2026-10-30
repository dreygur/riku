use super::*;

#[test]
fn docker_identity_uses_platform_name() {
    let json = r#"{"Client":{"Version":"27.3.1"},"Server":{"Platform":{"Name":"Docker Engine - Community"},"Version":"27.3.1"}}"#;
    let id = parse_identity("docker", json).unwrap();
    assert_eq!(
        (id.name.as_str(), id.version.as_str()),
        ("Docker Engine - Community", "27.3.1")
    );
}

#[test]
fn engine_without_product_name_shows_binary_name() {
    let json = r#"{"Client":{"Version":"5.2.2"},"Server":{"Version":"5.2.2"}}"#;
    assert_eq!(parse_identity("podman", json).unwrap().name, "podman");
}

#[test]
fn client_version_is_the_fallback_when_no_server_answers() {
    let json = r#"{"Client":{"Version":"1.7.0"}}"#;
    assert_eq!(parse_identity("nerdctl", json).unwrap().version, "1.7.0");
}

#[test]
fn rejects_version_output_without_any_version() {
    let err = parse_identity("docker", r#"{"Server":{}}"#).unwrap_err();
    assert!(err.to_string().contains("version"), "{err}");
}
