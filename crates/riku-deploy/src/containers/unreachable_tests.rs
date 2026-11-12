use std::fs;
use std::os::unix::fs::PermissionsExt;

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
    let engine = tmp.path().join("no-such-engine").display().to_string();
    crate::config::settings::save_key(&server.paths, "engine", "program", Some(&engine)).unwrap();
    (tmp, server)
}

fn server_with_dying_engine() -> (tempfile::TempDir, MockServer) {
    let tmp = tempfile::tempdir().unwrap();
    let server = mock_server(tmp.path(), "myapp", DB_AND_WEB, "");
    let marker = tmp.path().join("engine-died");
    let script = format!(
        "#!/bin/sh\n\
         case \"$2\" in --help) exit 0;; esac\n\
         if [ -f '{}' ]; then\n\
             case \"$1\" in\n\
                 version)\n\
                     echo 'engine daemon not running' >&2\n\
                     exit 1\n\
                     ;;\n\
             esac\n\
         fi\n\
         case \"$1\" in\n\
             version)\n\
                 echo '{{\"Client\":{{\"Version\":\"1\"}}}}'\n\
                 exit 0\n\
                 ;;\n\
             ps)\n\
                 touch '{}'\n\
                 echo 'connection reset by peer' >&2\n\
                 exit 1\n\
                 ;;\n\
             *)\n\
                 exit 0\n\
                 ;;\n\
         esac\n",
        marker.display(),
        marker.display()
    );
    let program = tmp.path().join("engine");
    fs::write(&program, script).unwrap();
    fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).unwrap();
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

#[test]
fn stop_reports_unreachable_when_the_engine_dies_after_open() {
    let app = myapp();
    let (_tmp, server) = server_with_dying_engine();
    let err = match stop_service(&app, &server.paths, None) {
        Ok(_) => panic!("stop succeeded when engine died after open"),
        Err(e) => e,
    };
    assert!(
        err.downcast_ref::<EngineUnreachable>().is_some(),
        "expected EngineUnreachable when daemon died after open, got: {err:#}"
    );
}

#[test]
fn restart_reports_unreachable_when_the_engine_dies_after_open() {
    let app = myapp();
    let (_tmp, server) = server_with_dying_engine();
    let err = match restart_service(&app, &server.paths, None) {
        Ok(_) => panic!("restart succeeded when engine died after open"),
        Err(e) => e,
    };
    assert!(
        err.downcast_ref::<EngineUnreachable>().is_some(),
        "expected EngineUnreachable when daemon died after open, got: {err:#}"
    );
}

#[test]
fn stop_of_unknown_service_is_not_engine_unreachable() {
    let tmp = tempfile::tempdir().unwrap();
    let server = mock_server(tmp.path(), "myapp", DB_AND_WEB, "");
    let app = myapp();
    let unknown = service("unknown");
    let err = match stop_service(&app, &server.paths, Some(&unknown)) {
        Ok(_) => panic!("stop of unknown service succeeded"),
        Err(e) => e,
    };
    assert!(
        err.downcast_ref::<EngineUnreachable>().is_none(),
        "expected non-EngineUnreachable error, got: {err:#}"
    );
    assert!(
        err.to_string().contains("unknown"),
        "error should name the unknown service: {err:#}"
    );
}
