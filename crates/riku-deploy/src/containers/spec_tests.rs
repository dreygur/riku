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

#[test]
fn riku_settings_in_app_env_do_not_reach_the_container() {
    let dir = tempfile::tempdir().unwrap();
    let (_, project) = project_in(dir.path(), "myapp", WEB);
    let app = AppName::try_from("myapp").unwrap();
    let env: BTreeMap<String, String> = [("RIKU_GHCR_TOKEN", "s3cret"), ("DATABASE_URL", "db")]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    let copy = riku_container::types::CopyIndex::new(1).unwrap();

    let spec = copy_spec(&app, &project, &project.services[0], copy, None, &env).unwrap();

    assert_eq!(spec.env.keys().collect::<Vec<_>>(), ["DATABASE_URL"]);
}
