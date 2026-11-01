use riku_container::types::CopyIndex;

use super::*;
use crate::containers::mock_state::{deploy, mock_server, myapp, DB_AND_WEB};
use crate::containers::{destroy_containers, restart_copy, RestartOutcome};

fn service(name: &str) -> ServiceName {
    ServiceName::try_from(name).unwrap()
}

#[test]
fn scale_refuses_storage_service() {
    let tmp = tempfile::tempdir().unwrap();
    let server = mock_server(tmp.path(), "myapp", DB_AND_WEB, "");

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
    let server = mock_server(tmp.path(), "myapp", DB_AND_WEB, "");
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
fn destroy_keeps_volumes() {
    let tmp = tempfile::tempdir().unwrap();
    let server = mock_server(tmp.path(), "myapp", DB_AND_WEB, "");
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
