use std::fs;
use std::os::unix::fs::PermissionsExt;

use crate::containers::mock_legacy::legacy_stack;
use crate::containers::mock_state::{deploy, mock_server, DB_AND_WEB};

const ENV: &str = "RIKU_DOMAINS=myapp.example.com\n";

#[test]
fn failed_migration_republishes_old_route() {
    let tmp = tempfile::tempdir().unwrap();
    let server = mock_server(tmp.path(), "myapp", DB_AND_WEB, ENV);
    legacy_stack(&server);
    server.mark("exits", "web");

    assert!(deploy(&server).is_err(), "deploy must fail when web exits");
    let site = server.site("myapp");
    assert!(
        site.contains("127.0.0.1:8080"),
        "old web copy must be routed again after rollback: {site:?}"
    );
}

#[test]
fn delete_failure_after_migration_is_a_warning() {
    let tmp = tempfile::tempdir().unwrap();
    let server = mock_server(tmp.path(), "myapp", DB_AND_WEB, ENV);
    legacy_stack(&server);

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
