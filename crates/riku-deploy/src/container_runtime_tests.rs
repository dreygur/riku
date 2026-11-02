use super::*;

#[test]
fn test_get_container_runtime() {
    let result = get_container_runtime();
    match result {
        Ok(name) => assert!(
            name == "docker" || name == "podman",
            "expected docker or podman: {name}"
        ),
        Err(e) => assert!(
            e.to_string()
                .contains("Neither Docker nor Podman is available"),
            "unexpected error message: {e}"
        ),
    }
}

#[test]
fn test_check_remote_runtime_format() {
    let result = check_remote_runtime("localhost");
    assert!(result.is_ok(), "check_remote_runtime failed: {result:?}");
}
