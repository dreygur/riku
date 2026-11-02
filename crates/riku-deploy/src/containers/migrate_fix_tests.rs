use std::fs;
use std::os::unix::fs::PermissionsExt;

use crate::containers::mock_legacy::legacy_stack;
use crate::containers::mock_state::{deploy, mock_server, DB_AND_WEB};

const ENV: &str = "RIKU_DOMAINS=myapp.example.com\n";

#[test]
fn failed_migration_republishes_old_route() {
    let tmp = tempfile::tempdir().unwrap();
    let server = mock_server(tmp.path(), "myapp", DB_AND_WEB, ENV);
    let _old = legacy_stack(&server);
    let port = _old.local_addr().unwrap().port();
    server.mark("exits", "web");

    assert!(deploy(&server).is_err(), "deploy must fail when web exits");
    let site = server.site("myapp");
    assert!(
        site.contains(&format!("127.0.0.1:{port}")),
        "old web copy must be routed again after rollback: {site:?}"
    );
}

#[test]
fn delete_failure_after_migration_is_a_warning() {
    let tmp = tempfile::tempdir().unwrap();
    let server = mock_server(tmp.path(), "myapp", DB_AND_WEB, ENV);
    let _old = legacy_stack(&server);

    // Root ignores directory permissions, so the delete would not fail.
    if nix::unistd::geteuid().is_root() {
        return;
    }
    fs::set_permissions(
        &server.paths.workers_available,
        fs::Permissions::from_mode(0o555),
    )
    .unwrap();

    let outcome = deploy(&server);

    fs::set_permissions(
        &server.paths.workers_available,
        fs::Permissions::from_mode(0o755),
    )
    .unwrap();

    assert!(
        outcome.is_ok(),
        "deploy must succeed with warning on discard failure: {outcome:?}"
    );
    assert_eq!(server.containers(), ["myapp-db-1", "myapp-web-1"]);
}

#[test]
fn rollback_reports_a_restored_worker_that_never_answers() {
    let tmp = tempfile::tempdir().unwrap();
    let server = mock_server(tmp.path(), "myapp", DB_AND_WEB, ENV);
    let old = legacy_stack(&server);
    let port = old.local_addr().unwrap().port();
    drop(old);
    server.mark("exits", "web");

    let err = deploy(&server).unwrap_err();
    let msg = format!("{err:#}");
    assert!(
        msg.contains("exited"),
        "error must contain migration failure: {msg}"
    );
    assert!(
        msg.contains("did not answer"),
        "error must contain health wait failure: {msg}"
    );
    let site = server.site("myapp");
    assert!(
        !site.contains(&format!("127.0.0.1:{port}")),
        "old route must not be republished when worker never answers: {site:?}"
    );
}
