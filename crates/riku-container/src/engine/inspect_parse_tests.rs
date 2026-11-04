use super::BadLabels;
use crate::engine::fixture::{inspect_json, ID, ID_B, IMAGE_ID};
use crate::engine::{ContainerInfo, ContainerState};

fn parse_inspect(json: &str) -> anyhow::Result<Vec<ContainerInfo>> {
    super::parse_inspect(json, BadLabels::Fail)
}

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
        "Name": "/myapp.web.2",
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

/// Captured from `docker inspect` (docker 29.8.2) of a container started with
/// a health check still in its start period and the riku.port and riku.restart
/// labels. Trimmed; id, pid and timestamps replaced with stock values.
const DOCKER_STARTING: &str = r#"[
    {
        "Id": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        "Name": "/myapp.web.1",
        "Image": "sha256:be2656061b536c1883c4034790e21d0a09c5aaaf51968b89368a3c0371ea2c9a",
        "State": {
            "Status": "running", "Running": true, "Pid": 1234, "ExitCode": 0, "Error": "",
            "StartedAt": "2026-10-30T13:27:17.273819223Z",
            "FinishedAt": "0001-01-01T00:00:00Z",
            "Health": {"Status": "starting", "FailingStreak": 0, "Log": []}
        },
        "Config": {
            "Image": "alpine",
            "Labels": {"riku.app": "myapp", "riku.port": "8080", "riku.restart": "on-failure"}
        }
    }
]"#;

/// Written from the podman documentation, not captured: podman reports the name
/// without a leading slash and `null` labels for a container started by hand.
const PODMAN_CREATED: &str = r#"[
    {
        "Id": "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210",
        "Name": "scratch",
        "Image": "be2656061b536c1883c4034790e21d0a09c5aaaf51968b89368a3c0371ea2c9a",
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
    assert_eq!(info.name, "myapp.web.2");
    assert_eq!(info.service.as_ref().unwrap().as_str(), "web");
    assert_eq!(info.copy.unwrap().get(), 2);
    assert_eq!(info.image, "shop/web:1.4");
    assert_eq!(info.state, ContainerState::Unhealthy);
}

#[test]
fn reads_app_port_restart_and_image_id_from_labels() {
    let json = inspect_json(
        ID,
        r#"{"Status":"running"}"#,
        r#"{"riku.app":"myapp","riku.port":"8080","riku.restart":"on-failure"}"#,
    );

    let info = &parse_inspect(&json).unwrap()[0];

    assert_eq!(info.app.as_deref(), Some("myapp"));
    assert_eq!(info.host_port, Some(8080));
    assert_eq!(info.restart.as_deref(), Some("on-failure"));
    assert_eq!(info.image_id, IMAGE_ID);
}

#[test]
fn docker_capture_in_health_start_period_is_starting_with_labels() {
    let info = &parse_inspect(DOCKER_STARTING).unwrap()[0];

    assert_eq!(info.state, ContainerState::Starting);
    assert_eq!(info.host_port, Some(8080));
    assert_eq!(info.restart.as_deref(), Some("on-failure"));
    assert_eq!(info.app.as_deref(), Some("myapp"));
}

#[test]
fn junk_port_label_is_none() {
    for junk in ["web", "70000", "0", ""] {
        let labels = format!(r#"{{"riku.port":"{junk}"}}"#);
        let json = inspect_json(ID, r#"{"Status":"running"}"#, &labels);

        let infos = parse_inspect(&json).unwrap();

        assert_eq!(infos[0].host_port, None, "port label {junk:?} was kept");
    }
}

#[test]
fn starting_health_is_starting() {
    let json = inspect_json(
        ID,
        r#"{"Status":"running","Health":{"Status":"starting"}}"#,
        "{}",
    );

    assert_eq!(
        parse_inspect(&json).unwrap()[0].state,
        ContainerState::Starting
    );
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

    assert_eq!(info.name, "myapp.web.2");
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

#[test]
fn listing_skips_a_container_with_an_unparseable_service_label() {
    let good = inspect_json(ID, r#"{"Status":"running"}"#, r#"{"riku.service":"web"}"#);
    let old = inspect_json(
        ID_B,
        r#"{"Status":"running"}"#,
        r#"{"riku.service":"api.v1"}"#,
    );
    let both = format!("[{},{}]", &good[1..good.len() - 1], &old[1..old.len() - 1]);

    let listed = super::parse_inspect(&both, BadLabels::Skip).unwrap();

    let ids: Vec<&str> = listed.iter().map(|info| info.id.as_str()).collect();
    assert_eq!(
        ids,
        [ID],
        "only the container with readable labels is listed"
    );
    assert!(
        parse_inspect(&both).is_err(),
        "an exact-name lookup must not read the unparseable container as absent"
    );
}

#[test]
fn path_traversal_app_label_is_skipped_by_listings_and_fails_lookups() {
    let crafted = inspect_json(ID, r#"{"Status":"running"}"#, r#"{"riku.app":"../.."}"#);

    let listed = super::parse_inspect(&crafted, BadLabels::Skip).unwrap();
    let err = parse_inspect(&crafted).unwrap_err().to_string();

    assert!(listed.is_empty(), "listed a crafted app label: {listed:?}");
    assert!(err.contains("riku.app"), "expected riku.app in {err:?}");
}
