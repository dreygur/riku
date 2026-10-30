use super::parse_inspect;
use crate::engine::fixture::{inspect_json, ID, ID_B};
use crate::engine::ContainerState;

/// Captured from `docker inspect` (docker 29.8.2) of a running alpine container
/// started with the three riku labels. The capture was trimmed and scrubbed:
/// host paths, ids, network details, the pid and the container name were
/// removed or replaced.
const DOCKER_RUNNING: &str = r#"[
    {
        "Id": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        "Created": "2026-10-30T13:27:17.215492635Z",
        "Path": "sleep",
        "Args": ["60"],
        "State": {
            "Status": "running",
            "Running": true,
            "Paused": false,
            "Restarting": false,
            "OOMKilled": false,
            "Dead": false,
            "Pid": 1234,
            "ExitCode": 0,
            "Error": "",
            "StartedAt": "2026-10-30T13:27:17.273819223Z",
            "FinishedAt": "0001-01-01T00:00:00Z"
        },
        "Image": "sha256:be2656061b536c1883c4034790e21d0a09c5aaaf51968b89368a3c0371ea2c9a",
        "Name": "/myapp-web-2",
        "RestartCount": 0,
        "HostConfig": {"RestartPolicy": {"Name": "no", "MaximumRetryCount": 0}, "AutoRemove": true},
        "Mounts": [],
        "Config": {
            "Hostname": "0123456789ab",
            "Env": ["PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"],
            "Cmd": ["sleep", "60"],
            "Image": "alpine",
            "Entrypoint": null,
            "Labels": {"riku.app": "myapp", "riku.copy": "2", "riku.service": "web"}
        },
        "NetworkSettings": {"Ports": {}, "Networks": {"bridge": {"IPAddress": "172.17.0.2"}}}
    }
]"#;

/// Written from the podman documentation, not captured: podman reports the name
/// without a leading slash and `null` labels for a container started by hand.
const PODMAN_CREATED: &str = r#"[
    {
        "Id": "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210",
        "Name": "scratch",
        "State": {"Status": "created", "ExitCode": 0},
        "Config": {"Image": "docker.io/library/alpine:latest", "Labels": null}
    }
]"#;

#[test]
fn reads_state_labels_and_image_from_an_unhealthy_container() {
    let json = inspect_json(
        ID,
        r#"{"Status":"running","ExitCode":0,"Health":{"Status":"unhealthy"}}"#,
        r#"{"riku.service":"web","riku.copy":"2"}"#,
    );

    let info = &parse_inspect(&json).unwrap()[0];

    assert_eq!(info.id.as_str(), ID);
    assert_eq!(info.name, "myapp-web-2");
    assert_eq!(info.service.as_ref().unwrap().as_str(), "web");
    assert_eq!(info.copy.unwrap().get(), 2);
    assert_eq!(info.image, "shop/web:1.4");
    assert_eq!(info.state, ContainerState::Unhealthy);
}

#[test]
fn exited_container_keeps_its_code() {
    let json = inspect_json(ID, r#"{"Status":"exited","ExitCode":137}"#, "{}");

    let info = &parse_inspect(&json).unwrap()[0];

    assert_eq!(info.state, ContainerState::Exited { code: 137 });
}

#[test]
fn restarting_and_unknown_statuses_are_kept() {
    let restarting = inspect_json(ID, r#"{"Status":"restarting","ExitCode":1}"#, "{}");
    let paused = inspect_json(ID, r#"{"Status":"paused","ExitCode":0}"#, "{}");

    assert_eq!(
        parse_inspect(&restarting).unwrap()[0].state,
        ContainerState::Restarting
    );
    assert_eq!(
        parse_inspect(&paused).unwrap()[0].state,
        ContainerState::Other("paused".to_string())
    );
}

#[test]
fn docker_capture_reads_with_unknown_fields_ignored() {
    let info = &parse_inspect(DOCKER_RUNNING).unwrap()[0];

    assert_eq!(info.name, "myapp-web-2");
    assert_eq!(info.service.as_ref().unwrap().as_str(), "web");
    assert_eq!(info.copy.unwrap().get(), 2);
    assert_eq!(info.image, "alpine");
    assert_eq!(info.state, ContainerState::Running);
}

#[test]
fn podman_shape_without_labels_has_no_service_or_copy() {
    let info = &parse_inspect(PODMAN_CREATED).unwrap()[0];

    assert_eq!(info.id.as_str(), ID_B);
    assert_eq!(info.name, "scratch");
    assert!(
        info.service.is_none() && info.copy.is_none(),
        "unlabelled container got service {:?} copy {:?}",
        info.service,
        info.copy
    );
    assert_eq!(info.state, ContainerState::Created);
}

#[test]
fn missing_or_malformed_fields_are_errors_naming_the_field() {
    let no_image =
        r#"[{"Id":"0123456789ab","Name":"/a","Config":{},"State":{"Status":"running"}}]"#;
    let no_status = r#"[{"Id":"0123456789ab","Name":"/a","Config":{"Image":"x"},"State":{}}]"#;
    let exited_without_code = inspect_json(ID, r#"{"Status":"exited"}"#, "{}");
    let bad_id = inspect_json("--privileged", r#"{"Status":"running"}"#, "{}");
    let bad_copy = inspect_json(ID, r#"{"Status":"running"}"#, r#"{"riku.copy":"0"}"#);
    let bad_service = inspect_json(ID, r#"{"Status":"running"}"#, r#"{"riku.service":"-x"}"#);

    for (json, field) in [
        (no_image, "Image"),
        (no_status, "Status"),
        (exited_without_code.as_str(), "ExitCode"),
        (bad_id.as_str(), "container id"),
        (bad_copy.as_str(), "riku.copy"),
        (bad_service.as_str(), "riku.service"),
        ("not json", "JSON"),
    ] {
        let err = parse_inspect(json).unwrap_err().to_string();
        assert!(err.contains(field), "expected {field:?} in {err:?}");
    }
}
