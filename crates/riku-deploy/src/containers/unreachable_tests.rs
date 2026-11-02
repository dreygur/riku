use std::fs;

use super::*;
use crate::containers::mock_state::{mock_server, myapp, MockServer, DB_AND_WEB};

fn service(name: &str) -> ServiceName {
    ServiceName::try_from(name).unwrap()
}

/// Each operation gets its own server: the deploy lock is a flock that a
/// process forked by a parallel test can briefly keep alive, so two lock
/// acquisitions on one lock file are racy.
fn server_without_engine() -> (tempfile::TempDir, MockServer) {
    let tmp = tempfile::tempdir().unwrap();
    let server = mock_server(tmp.path(), "myapp", DB_AND_WEB, "");
    fs::write(
        &server.paths.container_engine_file,
        tmp.path().join("no-such-engine").display().to_string(),
    )
    .unwrap();
    (tmp, server)
}

fn assert_unreachable<T>(res: anyhow::Result<T>, op: &str) {
    match res {
        Ok(_) => panic!("{op} succeeded without engine"),
        Err(err) => {
            assert!(
                err.downcast_ref::<EngineUnreachable>().is_some(),
                "{op}: expected EngineUnreachable, got: {err:#}"
            );
        }
    }
}

#[test]
fn container_operations_fail_when_engine_unreachable() {
    let app = myapp();

    let (_tmp, server) = server_without_engine();
    assert_unreachable(
        restart_service(&app, &server.paths, None),
        "restart_service",
    );
    let (_tmp, server) = server_without_engine();
    assert_unreachable(stop_service(&app, &server.paths, None), "stop_service");
    let (_tmp, server) = server_without_engine();
    assert_unreachable(
        scale_service(&app, &server.paths, &service("web"), 2),
        "scale_service",
    );
    let (_tmp, server) = server_without_engine();
    assert_unreachable(
        destroy_containers(&app, &server.paths),
        "destroy_containers",
    );
}
