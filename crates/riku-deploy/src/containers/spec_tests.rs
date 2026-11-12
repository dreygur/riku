use std::collections::BTreeMap;

use riku_container::compose::ComposeProject;
use riku_container::types::{labels, HostIp, PortBinding, Protocol, RestartPolicy};

use super::*;
use crate::mock_engine::project_in;

const WEB: &str = "
services:
  web:
    image: nginx:1
    ports: [\"8080:80\"]
";

fn spec_for(project: &ComposeProject, host_port: Option<u16>) -> riku_container::engine::RunSpec {
    let app = AppName::try_from("myapp").unwrap();
    let copy = riku_container::types::CopyIndex::new(1).unwrap();
    copy_spec(
        &app,
        project,
        &project.services[0],
        copy,
        host_port,
        &BTreeMap::new(),
    )
    .unwrap()
}

#[test]
fn routed_copy_gets_its_own_port() {
    let dir = tempfile::tempdir().unwrap();
    let (_, project) = project_in(dir.path(), "myapp", WEB);

    let spec = spec_for(&project, Some(5123));

    assert_eq!(
        spec.ports,
        vec![PortBinding {
            host_ip: HostIp::Loopback,
            host_port: Some(5123),
            container_port: 80,
            protocol: Protocol::Tcp,
        }]
    );
    assert_eq!(spec.labels[labels::PORT], "5123");
    assert_eq!(spec.labels[labels::APP], "myapp");
    assert_eq!(spec.labels[labels::COPY], "1");
}

#[test]
fn restart_label_maps_policy() {
    let dir = tempfile::tempdir().unwrap();
    let (_, mut project) = project_in(dir.path(), "myapp", WEB);
    project.services[0].restart = RestartPolicy::OnFailure {
        max_retries: Some(3),
    };

    let spec = spec_for(&project, Some(5123));

    assert_eq!(spec.labels[labels::RESTART], "on-failure:3");
}

#[test]
fn dockerfile_app_copy_is_labelled_unless_stopped() {
    let project = riku_container::compose::single_image_project(
        std::path::Path::new("/srv/myapp"),
        std::path::Path::new("Dockerfile"),
    )
    .unwrap();

    let spec = spec_for(&project, None);

    assert_eq!(
        spec.labels[labels::RESTART],
        "unless-stopped",
        "the supervisor reads this label to decide whether a crashed copy restarts"
    );
}

#[test]
fn compose_label_cannot_join_another_compose_project() {
    let dir = tempfile::tempdir().unwrap();
    let compose = "
services:
  web:
    image: nginx:1
    labels:
      com.docker.compose.project: victim
      team: blue
";
    let (_, project) = project_in(dir.path(), "myapp", compose);

    let spec = spec_for(&project, None);

    assert!(!spec.labels.contains_key(labels::COMPOSE_PROJECT));
    assert_eq!(spec.labels["team"], "blue");
}

/// A built `web` and a pulled `redis` with its own `environment`.
const BUILT_AND_PULLED: &str = "
services:
  redis:
    image: redis:7-alpine
    environment:
      REDIS_ARGS: --save 60 1
  web:
    build: ./web
";

/// The run spec of `service` in [`BUILT_AND_PULLED`] with an app env that
/// holds a riku setting and an app variable.
fn built_and_pulled_spec(service: &str) -> riku_container::engine::RunSpec {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("myapp").join("web")).unwrap();
    let (_, project) = project_in(dir.path(), "myapp", BUILT_AND_PULLED);
    let app = AppName::try_from("myapp").unwrap();
    let env: BTreeMap<String, String> = [("RIKU_GHCR_TOKEN", "s3cret"), ("DATABASE_URL", "db")]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    let def = project
        .services
        .iter()
        .find(|s| s.name.as_str() == service)
        .unwrap();
    let copy = riku_container::types::CopyIndex::new(1).unwrap();
    copy_spec(&app, &project, def, copy, None, &env).unwrap()
}

/// A routed `web` pulled from a registry, beside a pulled `redis`.
const PULLED_WEB_AND_REDIS: &str = "
services:
  redis:
    image: redis:7-alpine
  web:
    image: ghcr.io/acme/web:1
    ports: [\"8000\"]
";

#[test]
fn a_pulled_routed_web_gets_the_app_env_and_redis_does_not() {
    let dir = tempfile::tempdir().unwrap();
    let (_, project) = project_in(dir.path(), "myapp", PULLED_WEB_AND_REDIS);
    let app = AppName::try_from("myapp").unwrap();
    let env: BTreeMap<String, String> = [("RIKU_GHCR_TOKEN", "s3cret"), ("DATABASE_URL", "db")]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    let copy = riku_container::types::CopyIndex::new(1).unwrap();
    let spec_of = |name: &str| {
        let def = project
            .services
            .iter()
            .find(|s| s.name.as_str() == name)
            .unwrap();
        copy_spec(&app, &project, def, copy, None, &env).unwrap()
    };

    assert_eq!(
        spec_of("web").env.keys().collect::<Vec<_>>(),
        ["DATABASE_URL"],
        "the routed web lacks the app env"
    );
    assert!(
        spec_of("redis").env.is_empty(),
        "app env reached the pulled redis"
    );
}

#[test]
fn a_built_service_gets_the_app_env_without_riku_settings() {
    let spec = built_and_pulled_spec("web");

    assert_eq!(spec.env.keys().collect::<Vec<_>>(), ["DATABASE_URL"]);
}

#[test]
fn a_pulled_service_gets_only_its_own_environment() {
    let spec = built_and_pulled_spec("redis");

    assert_eq!(
        spec.env.keys().collect::<Vec<_>>(),
        ["REDIS_ARGS"],
        "app env reached a pulled image"
    );
}
