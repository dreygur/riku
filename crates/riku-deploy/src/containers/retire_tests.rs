use std::fs;

use crate::containers::mock_state::{deploy, mock_server, DB_AND_WEB};
use crate::containers::mock_workers::procfile_workers;

const ENV: &str = "RIKU_DOMAINS=myapp.example.com\n";

#[test]
fn switching_a_procfile_app_retires_its_workers() {
    let tmp = tempfile::tempdir().unwrap();
    let server = mock_server(tmp.path(), "myapp", DB_AND_WEB, ENV);
    let _workers = procfile_workers(&server);

    let outcome = deploy(&server);
    assert!(
        outcome.is_ok(),
        "deploy must succeed when switching procfile app: {outcome:?}"
    );

    let enabled_toml = server.paths.workers_enabled.join("myapp@web.1.toml");
    let enabled_migrating = server
        .paths
        .workers_enabled
        .join("myapp@web.1.toml.migrating");
    assert!(
        fs::symlink_metadata(&enabled_toml).is_err(),
        "myapp@web.1.toml must not exist in workers_enabled"
    );
    assert!(
        fs::symlink_metadata(&enabled_migrating).is_err(),
        "myapp@web.1.toml.migrating must not exist in workers_enabled"
    );

    let available_toml = server.paths.workers_available.join("myapp@web.1.toml");
    assert!(
        !available_toml.exists(),
        "workers_available/myapp@web.1.toml must be gone"
    );

    assert_eq!(
        server.containers(),
        ["myapp.db.1", "myapp.web.1"],
        "server containers must match expected containers"
    );
}

#[test]
fn failed_rollout_keeps_procfile_workers() {
    let tmp = tempfile::tempdir().unwrap();
    let server = mock_server(tmp.path(), "myapp", DB_AND_WEB, ENV);
    let _workers = procfile_workers(&server);
    server.mark("exits", "web");

    let outcome = deploy(&server);
    assert!(
        outcome.is_err(),
        "deploy must fail when web container exits: {outcome:?}"
    );

    let enabled_toml = server.paths.workers_enabled.join("myapp@web.1.toml");
    let enabled_migrating = server
        .paths
        .workers_enabled
        .join("myapp@web.1.toml.migrating");
    assert!(
        enabled_toml.exists(),
        "myapp@web.1.toml must still exist in workers_enabled"
    );
    assert!(
        fs::symlink_metadata(&enabled_migrating).is_err(),
        "no .migrating must remain in workers_enabled"
    );
}
