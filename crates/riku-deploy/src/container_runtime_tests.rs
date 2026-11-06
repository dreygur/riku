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
fn remote_version_check_times_out_the_connect_and_keeps_the_host_an_operand() {
    let args = remote_version_args("-oProxyCommand=evil", "docker");
    let args: Vec<&str> = args.iter().map(|a| a.to_str().unwrap()).collect();
    assert_eq!(
        args,
        [
            "-o",
            "ConnectTimeout=10",
            "--",
            "-oProxyCommand=evil",
            "docker --version"
        ]
    );
}
