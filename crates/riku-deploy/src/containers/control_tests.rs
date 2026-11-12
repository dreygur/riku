use riku_container::types::CopyIndex;

use super::*;
use crate::containers::mock_state::{deploy, mock_server_with_domain, myapp, DB_AND_WEB};
use crate::containers::{destroy_containers, restart_copy, RestartOutcome};

fn service(name: &str) -> ServiceName {
    ServiceName::try_from(name).unwrap()
}

#[test]
fn scale_refuses_storage_service() {
    let tmp = tempfile::tempdir().unwrap();
    let server = mock_server_with_domain(tmp.path(), "myapp", DB_AND_WEB, "myapp.example.com");

    let err = scale_service(&myapp(), &server.paths, &service("db"), 2).unwrap_err();

    let message = format!("{err:#}");
    assert!(
        message.contains("volume data") && message.contains("cannot scale"),
        "{message}"
    );
    assert!(
        server.calls().iter().all(|l| !l.starts_with("run")),
        "a copy was started: {:?}",
        server.calls()
    );
}

#[test]
fn restart_copy_reports_busy() {
    let tmp = tempfile::tempdir().unwrap();
    let server = mock_server_with_domain(tmp.path(), "myapp", DB_AND_WEB, "myapp.example.com");
    let _deploy = crate::lock::acquire("myapp", &server.paths).unwrap();
    let copy = CopyIndex::new(1).unwrap();

    let outcome = restart_copy(&myapp(), &server.paths, &service("web"), copy).unwrap();

    assert_eq!(outcome, RestartOutcome::Busy);
    assert_eq!(
        server.calls(),
        Vec::<String>::new(),
        "the engine was called"
    );
}

#[test]
fn restart_copy_replaces_a_crashed_copy() {
    let tmp = tempfile::tempdir().unwrap();
    let server = mock_server_with_domain(tmp.path(), "myapp", DB_AND_WEB, "myapp.example.com");
    deploy(&server).unwrap();
    server.crash("myapp.web.1");
    let before = server.calls().len();

    let outcome = restart_copy(
        &myapp(),
        &server.paths,
        &service("web"),
        CopyIndex::new(1).unwrap(),
    )
    .unwrap();

    assert_eq!(outcome, RestartOutcome::Restarted);
    let calls = server.calls()[before..].to_vec();
    assert!(
        calls
            .iter()
            .any(|l| l.starts_with("run") && l.contains("myapp.web.1")),
        "no fresh web copy was started: {calls:#?}"
    );
    assert_eq!(server.containers(), ["myapp.db.1", "myapp.web.1"]);
}

#[test]
fn restart_copy_leaves_a_user_stopped_service_stopped() {
    let tmp = tempfile::tempdir().unwrap();
    let server = mock_server_with_domain(tmp.path(), "myapp", DB_AND_WEB, "myapp.example.com");
    deploy(&server).unwrap();
    stop_service(&myapp(), &server.paths, Some(&service("web"))).unwrap();
    let after_stop = server.calls().len();

    let outcome = restart_copy(
        &myapp(),
        &server.paths,
        &service("web"),
        CopyIndex::new(1).unwrap(),
    )
    .unwrap();

    assert_eq!(outcome, RestartOutcome::Gone);
    let calls = server.calls()[after_stop..].to_vec();
    assert!(
        calls.iter().all(|l| !l.starts_with("run")),
        "the supervisor undid the stop: {calls:#?}"
    );
    assert_eq!(server.containers(), ["myapp.db.1"]);
}

#[test]
fn destroy_keeps_volumes() {
    let tmp = tempfile::tempdir().unwrap();
    let server = mock_server_with_domain(tmp.path(), "myapp", DB_AND_WEB, "myapp.example.com");
    deploy(&server).unwrap();
    let before = server.calls().len();

    destroy_containers(&myapp(), &server.paths).unwrap();

    let calls = server.calls()[before..].to_vec();
    assert_eq!(
        calls.iter().filter(|l| l.starts_with("rm ")).count(),
        2,
        "{calls:#?}"
    );
    assert!(
        calls.iter().any(|l| l == "network rm riku-myapp"),
        "{calls:#?}"
    );
    assert!(calls.iter().all(|l| !l.contains("volume")), "{calls:#?}");
    assert!(server.containers().is_empty(), "{:?}", server.containers());
}

#[test]
fn destroy_removes_the_images_riku_built_and_keeps_pulled_ones() {
    let tmp = tempfile::tempdir().unwrap();
    let compose = "
services:
  web:
    build: ./web
    ports: [\"8000\"]
    depends_on: [redis]
    healthcheck:
      test: [\"CMD\", \"true\"]
  redis:
    image: redis:7-alpine
";
    let server = mock_server_with_domain(tmp.path(), "myapp", compose, "myapp.example.com");
    let web_dir = server.paths.app_root.join("myapp").join("web");
    std::fs::create_dir_all(&web_dir).unwrap();
    std::fs::write(web_dir.join("Dockerfile"), "FROM scratch\n").unwrap();
    deploy(&server).unwrap();
    let before = server.calls().len();

    destroy_containers(&myapp(), &server.paths).unwrap();

    let calls = server.calls()[before..].to_vec();
    let built = crate::containers::images::build_tag(&myapp(), &service("web"));
    assert!(
        calls.iter().any(|l| l == &format!("image rm {built}")),
        "the built image {built} was not removed: {calls:#?}"
    );
    assert!(
        !calls
            .iter()
            .any(|l| l.starts_with("image rm") && l.contains("redis")),
        "a pulled image was removed: {calls:#?}"
    );
}

#[test]
fn destroy_containers_refuses_when_deploy_in_progress() {
    let tmp = tempfile::tempdir().unwrap();
    let server = mock_server_with_domain(tmp.path(), "myapp", DB_AND_WEB, "myapp.example.com");
    deploy(&server).unwrap();
    let before = server.calls().len();
    let _lock = crate::lock::acquire("myapp", &server.paths).unwrap();

    let err = destroy_containers(&myapp(), &server.paths).unwrap_err();

    assert!(
        matches!(
            err.downcast_ref::<crate::error::DeployError>(),
            Some(crate::error::DeployError::DeployInProgress(_))
        ),
        "expected DeployInProgress, got: {err:?}"
    );
    assert_eq!(
        server.calls().len(),
        before,
        "engine was called after lock contention: {:?}",
        &server.calls()[before..]
    );
}

#[test]
fn restart_copy_clears_route_when_restart_fails() {
    let tmp = tempfile::tempdir().unwrap();
    let server = mock_server_with_domain(tmp.path(), "myapp", DB_AND_WEB, "myapp.example.com");
    deploy(&server).unwrap();
    let site = server.site("myapp");
    assert!(
        site.contains("reverse_proxy"),
        "expected reverse_proxy in site, got: {site}"
    );

    let port = site
        .split("127.0.0.1:")
        .nth(1)
        .and_then(|rest| rest.split_whitespace().next())
        .expect("could not find host port in site");

    server.crash("myapp.web.1");
    server.mark("exits", "web");

    let err = restart_copy(
        &myapp(),
        &server.paths,
        &service("web"),
        CopyIndex::new(1).unwrap(),
    )
    .unwrap_err();

    let message = format!("{err:#}");
    assert!(
        message.contains("did not become healthy") || message.contains("exited with code 1"),
        "expected health or exit failure, got: {message}"
    );

    let new_site = server.site("myapp");
    assert!(
        !new_site.contains(port),
        "site still points at removed copy's port {port}: {new_site}"
    );
}

#[test]
fn restart_service_refuses_when_dependency_has_no_running_copies() {
    let tmp = tempfile::tempdir().unwrap();
    let server = mock_server_with_domain(tmp.path(), "myapp", DB_AND_WEB, "myapp.example.com");
    deploy(&server).unwrap();

    stop_service(&myapp(), &server.paths, Some(&service("db"))).unwrap();
    let after_stop = server.calls().len();

    let err = restart_service(&myapp(), &server.paths, Some(&service("web"))).unwrap_err();

    let message = format!("{err:#}");
    assert!(
        message.contains("no running copies"),
        "expected 'no running copies' in error, got: {message}"
    );
    let runs_after_stop = server.calls()[after_stop..]
        .iter()
        .filter(|l| l.starts_with("run"))
        .count();
    assert_eq!(
        runs_after_stop,
        0,
        "a copy was started after stop: {:?}",
        &server.calls()[after_stop..]
    );
}

#[test]
fn stop_service_stops_dropped_services_first() {
    let tmp = tempfile::tempdir().unwrap();
    let server = mock_server_with_domain(tmp.path(), "myapp", DB_AND_WEB, "myapp.example.com");
    deploy(&server).unwrap();

    let only_db = "
services:
  db:
    image: postgres:16
    volumes: [\"data:/var/lib/postgresql/data\"]
    healthcheck:
      test: [\"CMD\", \"true\"]
volumes:
  data:
";
    std::fs::write(
        server.paths.app_root.join("myapp").join("compose.yaml"),
        only_db,
    )
    .unwrap();

    stop_service(&myapp(), &server.paths, None).unwrap();

    let calls = server.calls();
    let web_rm = calls
        .iter()
        .position(|l| l.starts_with("rm ") && l.ends_with("0002"))
        .unwrap_or_else(|| panic!("no rm for web container ending in 0002: {calls:#?}"));
    let db_rm = calls
        .iter()
        .position(|l| l.starts_with("rm ") && l.ends_with("0001"))
        .unwrap_or_else(|| panic!("no rm for db container ending in 0001: {calls:#?}"));
    assert!(
        web_rm < db_rm,
        "dropped web service was not stopped before db: web_rm={web_rm}, db_rm={db_rm}, calls={calls:#?}"
    );
}
