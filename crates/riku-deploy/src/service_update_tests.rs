use std::time::Duration;

use crate::containers::mock_state::{deploy, mock_server_with_domain, DB_AND_WEB};
use crate::service_update::pull_service;

const COMPOSE: &str = "
services:
  db:
    image: postgres:16
  web:
    build: .
";

#[test]
fn same_digest_does_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let server = mock_server_with_domain(tmp.path(), "myapp", DB_AND_WEB, "myapp.example.com");
    deploy(&server).unwrap();

    let start_calls = server.calls().len();
    let start_containers = server.containers();

    pull_service("myapp", &server.paths, "web").unwrap();

    let calls_after = &server.calls()[start_calls..];
    let pull_call = calls_after.iter().find(|c| c.starts_with("pull"));
    assert_eq!(
        pull_call.map(String::as_str),
        Some("pull nginx:alpine"),
        "expected pull nginx:alpine in calls: {calls_after:#?}"
    );

    let run_or_rm = calls_after
        .iter()
        .find(|c| c.starts_with("run") || c.starts_with("rm "));
    assert!(
        run_or_rm.is_none(),
        "unexpected run or rm in calls: {calls_after:#?}"
    );

    assert_eq!(
        server.containers(),
        start_containers,
        "containers changed when image was unchanged: calls={calls_after:#?}"
    );
}

#[test]
fn new_digest_replaces_service() {
    let tmp = tempfile::tempdir().unwrap();
    let server = mock_server_with_domain(tmp.path(), "myapp", DB_AND_WEB, "myapp.example.com");
    deploy(&server).unwrap();

    let start_calls = server.calls().len();
    std::fs::write(server.state.join("image-id"), "sha256:y\n").unwrap();

    pull_service("myapp", &server.paths, "web").unwrap();

    let calls_after = &server.calls()[start_calls..];
    let run_lines: Vec<&String> = calls_after
        .iter()
        .filter(|c| c.starts_with("run"))
        .collect();
    assert_eq!(
        run_lines.len(),
        1,
        "expected exactly one run call after update: {calls_after:#?}"
    );
    assert!(
        run_lines[0].contains("riku.service=web"),
        "run call must be for service web: {}",
        run_lines[0]
    );

    let rm_lines: Vec<&String> = calls_after
        .iter()
        .filter(|c| c.starts_with("rm "))
        .collect();
    assert!(
        !rm_lines.is_empty(),
        "expected at least one rm line: {calls_after:#?}"
    );

    assert!(
        !run_lines.iter().any(|c| c.contains("riku.service=db")),
        "no run line for db expected: {run_lines:#?}"
    );

    let current_containers = server.containers();
    assert!(
        current_containers.contains(&"myapp.db.1".to_string()),
        "db container must still be in containers: {current_containers:#?}"
    );
    assert!(
        !current_containers.contains(&"myapp.web.1".to_string()),
        "old web container must be gone: {current_containers:#?}"
    );
}

#[test]
fn built_service_is_skipped_without_pull() {
    let tmp = tempfile::tempdir().unwrap();
    let server = mock_server_with_domain(tmp.path(), "myapp", COMPOSE, "myapp.example.com");
    deploy(&server).unwrap();

    let start = server.calls().len();
    pull_service("myapp", &server.paths, "web").unwrap();

    let calls = &server.calls()[start..];
    assert!(
        !calls.iter().any(|c| c.starts_with("pull")),
        "expected no call starting with pull, got calls: {calls:#?}"
    );
}

#[test]
fn built_service_is_skipped_before_lock_and_engine() {
    let tmp = tempfile::tempdir().unwrap();
    let server = mock_server_with_domain(tmp.path(), "myapp", COMPOSE, "myapp.example.com");
    deploy(&server).unwrap();

    let _lock = crate::lock::acquire("myapp", &server.paths).unwrap();
    let engine = tmp.path().join("no-such-engine").display().to_string();
    crate::config::settings::save_key(&server.paths, "engine", "program", Some(&engine)).unwrap();

    let start = std::time::Instant::now();
    let res = pull_service("myapp", &server.paths, "web");
    assert!(
        res.is_ok(),
        "expected pull_service to return Ok for built service, got: {res:?}"
    );
    assert!(
        start.elapsed() < Duration::from_millis(500),
        "pull_service took too long ({:?}), expected to return quickly without contending lock",
        start.elapsed()
    );
}
