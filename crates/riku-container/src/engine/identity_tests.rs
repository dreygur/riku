use std::fs;

use super::*;
use crate::engine::fake::fake_engine;

#[test]
fn docker_identity_uses_platform_name() {
    // Captured from `docker version --format json` (Docker 29.8.2), then
    // trimmed and scrubbed: the context name and kernel version are generic.
    let json = r#"{"Client":{"Version":"29.8.2","ApiVersion":"1.56","DefaultAPIVersion":"1.56","GitCommit":"7fc2dff","GoVersion":"go1.26.8","Os":"darwin","Arch":"arm64","BuildTime":"Sun Oct 11 12:42:20 2026","Context":"default"},"Server":{"Platform":{"Name":"Docker Engine - Community"},"Version":"29.8.2","ApiVersion":"1.56","MinAPIVersion":"1.40","Os":"linux","Arch":"arm64","Components":[{"Name":"Engine","Version":"29.8.2","Details":{"ApiVersion":"1.56","Arch":"arm64","BuildTime":"Wed Sep 30 19:32:49 2026","Experimental":"true","GitCommit":"8af9fe3","GoVersion":"go1.26.8","KernelVersion":"6.1.0-amd64","MinAPIVersion":"1.40","Module":"github.com/moby/moby/v2","ModuleVersion":"v2.0.0+unknown","Os":"linux"}},{"Name":"containerd","Version":"v2.3.6","Details":{"GitCommit":"ee2735368117d2eb259779949d5e75cdafec9761"}},{"Name":"runc","Version":"1.5.2","Details":{"GitCommit":"29dd3dc2b13b4123162e5fe132504bb4b15569f1"}},{"Name":"docker-init","Version":"0.19.0","Details":{"GitCommit":"de40ad0"}}],"GitCommit":"8af9fe3","GoVersion":"go1.26.8","KernelVersion":"6.1.0-amd64","Experimental":true,"BuildTime":"2026-09-30T19:32:49.000000000+00:00"}}"#;
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

#[test]
fn ping_succeeds_for_healthy_engine_and_fails_when_version_fails() {
    let dir = tempfile::tempdir().unwrap();
    let flag = dir.path().join("fail");
    let script = format!(
        "case \"$2\" in --help) exit 0;; esac\n\
         if [ -f '{}' ]; then\n\
             echo 'daemon stopped' >&2\n\
             exit 1\n\
         fi\n\
         case \"$1\" in version) echo '{{\"Client\":{{\"Version\":\"1\"}}}}'; exit 0;; esac\n\
         exit 0",
        flag.display()
    );
    let binary = EngineBinary {
        program: fake_engine(dir.path(), "engine", &script),
        binary_name: "engine".to_string(),
    };
    let engine = Engine::open(binary, Vec::new()).unwrap();

    let ping_healthy = engine.ping();
    assert!(
        ping_healthy.is_ok(),
        "healthy engine ping must succeed, got: {ping_healthy:?}"
    );

    fs::write(&flag, "").unwrap();

    let ping_failed = engine.ping();
    assert!(
        ping_failed.is_err(),
        "ping must fail when daemon version stops answering"
    );
}
