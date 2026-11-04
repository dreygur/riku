use std::fs;

use crate::containers::mock_legacy::{legacy_id, legacy_stack, write_volume_state};
use crate::containers::mock_state::{deploy, mock_server, DB_AND_WEB};

const ENV: &str = "RIKU_DOMAINS=myapp.example.com\n";

#[test]
fn migration_reuses_volume_names() {
    let tmp = tempfile::tempdir().unwrap();
    let server = mock_server(tmp.path(), "myapp", DB_AND_WEB, ENV);
    let _old = legacy_stack(&server);

    deploy(&server).unwrap();

    let calls = server.calls();
    let db_run = calls
        .iter()
        .find(|l| l.starts_with("run") && l.contains("myapp-db-1"))
        .unwrap_or_else(|| panic!("no db run call found in {calls:#?}"));
    assert!(
        db_run.contains("myapp_data:"),
        "db run call must mount myapp_data: {db_run}"
    );

    let enabled_toml = server.paths.workers_enabled.join("myapp@web.1.toml");
    let enabled_migrating = server
        .paths
        .workers_enabled
        .join("myapp@web.1.toml.migrating");
    assert!(
        fs::symlink_metadata(&enabled_toml).is_err(),
        "legacy worker .toml link must be gone from workers_enabled"
    );
    assert!(
        fs::symlink_metadata(&enabled_migrating).is_err(),
        "legacy worker .toml.migrating link must be gone from workers_enabled"
    );

    assert_eq!(server.containers(), ["myapp-db-1", "myapp-web-1"]);

    let legacy_db_id = legacy_id(1);
    let legacy_web_id = legacy_id(2);
    assert!(
        !server.state.join(format!("{legacy_db_id}.state")).exists(),
        "legacy db container state must be removed"
    );
    assert!(
        !server.state.join(format!("{legacy_web_id}.state")).exists(),
        "legacy web container state must be removed"
    );

    assert!(
        calls.iter().any(|l| l.contains("network rm myapp_default")),
        "network rm myapp_default must have been called: {calls:#?}"
    );
}

#[test]
fn declared_name_of_other_app_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let env = format!("{ENV}COMPOSE_PROJECT_NAME=Sh.op\n");
    let server = mock_server(tmp.path(), "myapp", DB_AND_WEB, &env);
    fs::create_dir_all(server.paths.app_root.join("shop")).unwrap();

    let err = deploy(&server).unwrap_err();

    let msg = format!("{err:#}");
    assert!(
        msg.contains("app 'shop'"),
        "error message must name the other app shop: {msg}"
    );

    let calls = server.calls();
    assert!(
        !calls.iter().any(|l| l.starts_with("run")),
        "no run command should have been executed: {calls:#?}"
    );
}

#[test]
fn unlabelled_volume_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let env = format!("{ENV}COMPOSE_PROJECT_NAME=shop\n");
    let server = mock_server(tmp.path(), "myapp", DB_AND_WEB, &env);
    write_volume_state(&server, "shop_data", "null");

    let err = deploy(&server).unwrap_err();

    let msg = format!("{err:#}");
    assert!(
        msg.contains("shop_data"),
        "error message must name shop_data: {msg}"
    );

    let calls = server.calls();
    assert!(
        !calls.iter().any(|l| l.starts_with("run")),
        "no run command should have been executed: {calls:#?}"
    );
}

#[test]
fn declared_name_adopts_its_compose_volume() {
    let tmp = tempfile::tempdir().unwrap();
    let env = format!("{ENV}COMPOSE_PROJECT_NAME=shop\n");
    let server = mock_server(tmp.path(), "myapp", DB_AND_WEB, &env);
    write_volume_state(
        &server,
        "shop_data",
        r#"{"com.docker.compose.project":"shop"}"#,
    );

    deploy(&server).unwrap();

    let calls = server.calls();
    assert!(
        calls
            .iter()
            .any(|l| l.starts_with("run") && l.contains("myapp-db-1") && l.contains("shop_data:")),
        "db copy does not mount shop_data: {calls:#?}"
    );
}

#[test]
fn failed_start_restores_old_stack() {
    let tmp = tempfile::tempdir().unwrap();
    let server = mock_server(tmp.path(), "myapp", DB_AND_WEB, ENV);
    let _old = legacy_stack(&server);
    server.mark("exits", "web");

    let err = deploy(&server).unwrap_err();
    assert!(
        format!("{err:#}").contains("the old stack was restored"),
        "error must indicate stack was restored: {err:#}"
    );

    let enabled_toml = server.paths.workers_enabled.join("myapp@web.1.toml");
    let enabled_migrating = server
        .paths
        .workers_enabled
        .join("myapp@web.1.toml.migrating");
    assert!(
        enabled_toml.exists(),
        "myapp@web.1.toml must be restored in workers_enabled"
    );
    assert!(
        !enabled_migrating.exists(),
        "no .toml.migrating must remain in workers_enabled"
    );

    for entry in fs::read_dir(&server.state).unwrap().flatten() {
        if entry.path().extension().is_some_and(|e| e == "labels") {
            let labels = fs::read_to_string(entry.path()).unwrap();
            assert!(
                !labels.contains("\"riku.app\""),
                "no container with riku labels should remain: {labels}"
            );
        }
    }

    let calls = server.calls();
    assert!(
        !calls.iter().any(|l| l.contains("volume rm")),
        "volume rm must not appear in logged calls: {calls:#?}"
    );
}

#[test]
fn migration_never_removes_volumes() {
    let tmp_ok = tempfile::tempdir().unwrap();
    let server_ok = mock_server(tmp_ok.path(), "myapp", DB_AND_WEB, ENV);
    let _old = legacy_stack(&server_ok);
    deploy(&server_ok).unwrap();

    let tmp_fail = tempfile::tempdir().unwrap();
    let server_fail = mock_server(tmp_fail.path(), "myapp", DB_AND_WEB, ENV);
    let _old = legacy_stack(&server_fail);
    server_fail.mark("exits", "web");
    let _ = deploy(&server_fail).unwrap_err();

    let mut all_calls = server_ok.calls();
    all_calls.extend(server_fail.calls());
    assert!(
        !all_calls.iter().any(|l| l.contains("volume rm")),
        "volume rm must never appear in any logged calls: {all_calls:#?}"
    );

    assert!(
        server_ok.state.join("vol-myapp_data").exists(),
        "vol-myapp_data must still exist after successful migration"
    );
    assert!(
        server_fail.state.join("vol-myapp_data").exists(),
        "vol-myapp_data must still exist after failed migration"
    );
}
