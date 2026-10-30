use super::*;

#[test]
fn docker_identity_uses_platform_name() {
    // Captured from `docker version --format json` (Docker 29.8.2).
    let json = r#"{"Client":{"Version":"29.8.2","ApiVersion":"1.56","DefaultAPIVersion":"1.56","GitCommit":"7fc2dff","GoVersion":"go1.26.8","Os":"darwin","Arch":"arm64","BuildTime":"Sun Oct 11 12:42:20 2026","Context":"orbstack"},"Server":{"Platform":{"Name":"Docker Engine - Community"},"Version":"29.8.2","ApiVersion":"1.56","MinAPIVersion":"1.40","Os":"linux","Arch":"arm64","Components":[{"Name":"Engine","Version":"29.8.2","Details":{"ApiVersion":"1.56","Arch":"arm64","BuildTime":"Wed Sep 30 19:32:49 2026","Experimental":"true","GitCommit":"8af9fe3","GoVersion":"go1.26.8","KernelVersion":"7.2.12-orbstack-00394-gbecdd6130068","MinAPIVersion":"1.40","Module":"github.com/moby/moby/v2","ModuleVersion":"v2.0.0+unknown","Os":"linux"}},{"Name":"containerd","Version":"v2.3.6","Details":{"GitCommit":"ee2735368117d2eb259779949d5e75cdafec9761"}},{"Name":"runc","Version":"1.5.2","Details":{"GitCommit":"29dd3dc2b13b4123162e5fe132504bb4b15569f1"}},{"Name":"docker-init","Version":"0.19.0","Details":{"GitCommit":"de40ad0"}}],"GitCommit":"8af9fe3","GoVersion":"go1.26.8","KernelVersion":"7.2.12-orbstack-00394-gbecdd6130068","Experimental":true,"BuildTime":"2026-09-30T19:32:49.000000000+00:00"}}"#;
    let id = parse_identity("docker", json).unwrap();
    assert_eq!(
        (id.name.as_str(), id.version.as_str()),
        ("Docker Engine - Community", "29.8.2")
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
