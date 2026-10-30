use super::{parse_inspect, ContainerState};
use crate::engine::fixture::{logged, logging_engine};
use crate::engine::ContainerId;

const ID: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const ID_B: &str = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";

/// Captured from `docker inspect` (docker 29.8.2) of a running alpine container
/// started with the three riku labels. Host paths, ids, network ids and the
/// container name were replaced; every other field is kept as the engine sent it.
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
            "Pid": 6861,
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
        "NetworkSettings": {"Ports": {}, "Networks": {"bridge": {"IPAddress": "192.168.215.2"}}}
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

fn container(id: &str, state: &str, labels: &str) -> String {
    format!(
        r#"[{{"Id":"{id}","Name":"/myapp-web-2","Config":{{"Image":"shop/web:1.4","Labels":{labels}}},"State":{state}}}]"#
    )
}

#[test]
fn reads_state_labels_and_image_from_an_unhealthy_container() {
    let json = container(
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
    let json = container(ID, r#"{"Status":"exited","ExitCode":137}"#, "{}");

    let info = &parse_inspect(&json).unwrap()[0];

    assert_eq!(info.state, ContainerState::Exited { code: 137 });
}

#[test]
fn restarting_and_unknown_statuses_are_kept() {
    let restarting = container(ID, r#"{"Status":"restarting","ExitCode":1}"#, "{}");
    let paused = container(ID, r#"{"Status":"paused","ExitCode":0}"#, "{}");

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
    assert!(info.service.is_none() && info.copy.is_none());
    assert_eq!(info.state, ContainerState::Created);
}

#[test]
fn missing_or_malformed_fields_are_errors_naming_the_field() {
    let no_image =
        r#"[{"Id":"0123456789ab","Name":"/a","Config":{},"State":{"Status":"running"}}]"#;
    let no_status = r#"[{"Id":"0123456789ab","Name":"/a","Config":{"Image":"x"},"State":{}}]"#;
    let exited_without_code = container(ID, r#"{"Status":"exited"}"#, "{}");
    let bad_id = container("--privileged", r#"{"Status":"running"}"#, "{}");
    let bad_copy = container(ID, r#"{"Status":"running"}"#, r#"{"riku.copy":"0"}"#);
    let bad_service = container(ID, r#"{"Status":"running"}"#, r#"{"riku.service":"-x"}"#);

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
fn list_filters_by_app_label_and_keeps_full_ids() {
    let dir = tempfile::tempdir().unwrap();
    let (engine, log) = logging_engine(dir.path(), &format!("printf '%s\\n' {ID} {ID_B}"));

    let ids = engine.list("myapp").unwrap();

    assert_eq!(
        logged(&log),
        ["ps -a -q --no-trunc --filter label=riku.app=myapp"]
    );
    assert_eq!(
        ids,
        [
            ContainerId::try_from(ID).unwrap(),
            ContainerId::try_from(ID_B).unwrap()
        ]
    );
}

#[test]
fn list_refuses_an_app_that_could_alter_the_filter() {
    let dir = tempfile::tempdir().unwrap();
    let (engine, log) = logging_engine(dir.path(), "");

    for app in ["x,label=a", "../x", "-x", ""] {
        assert!(engine.list(app).is_err(), "accepted {app:?}");
    }
    assert!(
        logged(&log).is_empty(),
        "engine was run: {:?}",
        logged(&log)
    );
}

#[test]
fn list_refuses_engine_output_that_is_not_an_id() {
    let dir = tempfile::tempdir().unwrap();
    let (engine, _log) = logging_engine(dir.path(), "echo --all");

    assert!(engine.list("myapp").is_err());
}

#[test]
fn inspect_asks_for_containers_by_full_id_and_skips_the_engine_for_none() {
    let dir = tempfile::tempdir().unwrap();
    let json = container(ID, r#"{"Status":"running"}"#, "{}").replace('\n', " ");
    let (engine, log) = logging_engine(dir.path(), &format!("echo '{json}'"));

    assert!(engine.inspect(&[]).unwrap().is_empty());
    assert!(logged(&log).is_empty(), "ran the engine for no ids");

    let infos = engine
        .inspect(&[ContainerId::try_from(ID).unwrap()])
        .unwrap();

    assert_eq!(logged(&log), [format!("inspect --type container {ID}")]);
    assert_eq!(infos[0].state, ContainerState::Running);
}
