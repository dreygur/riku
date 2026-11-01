use crate::containers::mock_state::{deploy, mock_server, MockServer, DB_AND_WEB};

const ENV: &str = "RIKU_DOMAINS=myapp.example.com\n";

fn position(calls: &[String], wanted: impl Fn(&str) -> bool) -> usize {
    calls
        .iter()
        .position(|line| wanted(line))
        .unwrap_or_else(|| panic!("no matching call in {calls:#?}"))
}

#[test]
fn deploy_starts_services_in_order() {
    let tmp = tempfile::tempdir().unwrap();
    let server = mock_server(tmp.path(), "myapp", DB_AND_WEB, ENV);
    server.mark("slow", "db");

    deploy(&server).unwrap();

    let calls = server.calls();
    let db_ready = position(&calls, |l| l == "ready myapp-db-1");
    let web_run = position(&calls, |l| {
        l.starts_with("run") && l.contains("myapp-web-1")
    });
    assert!(
        db_ready < web_run,
        "web started before db was healthy: {calls:#?}"
    );
    assert_eq!(server.containers(), ["myapp-db-1", "myapp-web-1"]);
}

#[test]
fn redeploy_keeps_old_copy_on_failure() {
    let tmp = tempfile::tempdir().unwrap();
    let server = mock_server(tmp.path(), "myapp", DB_AND_WEB, ENV);
    deploy(&server).unwrap();
    let site_before = server.site("myapp");
    assert!(site_before.contains("reverse_proxy"), "{site_before}");
    server.mark("exits", "web");

    let err = deploy(&server).unwrap_err();

    assert!(format!("{err:#}").contains("exited with code 1"), "{err:#}");
    assert!(
        server.containers().contains(&"myapp-web-1".to_string()),
        "old web copy is gone: {:?}",
        server.containers()
    );
    assert!(
        !server.containers().contains(&"myapp-web-2".to_string()),
        "failed copy left behind: {:?}",
        server.containers()
    );
    assert_eq!(server.site("myapp"), site_before, "router config changed");
}

fn with_release(server: &MockServer) {
    let app_path = server.paths.app_root.join("myapp");
    std::fs::write(app_path.join("Procfile"), "release: echo migrate\n").unwrap();
}

#[test]
fn release_runs_in_new_image_before_rollout() {
    let tmp = tempfile::tempdir().unwrap();
    let server = mock_server(tmp.path(), "myapp", DB_AND_WEB, ENV);
    with_release(&server);

    deploy(&server).unwrap();

    let calls = server.calls();
    let release_run = position(&calls, |l| {
        l.starts_with("run") && l.contains("myapp-release")
    });
    let release_wait = position(&calls, |l| l.starts_with("wait"));
    let release_rm = position(&calls, |l| l.starts_with("rm"));
    let web_run = position(&calls, |l| {
        l.starts_with("run") && l.contains("myapp-web-1")
    });
    let run_line = &calls[release_run];
    assert!(
        run_line.contains("--entrypoint /bin/sh")
            && run_line.contains("nginx:alpine -c echo migrate"),
        "release did not run through the shell in the web image: {run_line}"
    );
    assert!(
        release_run < release_wait && release_wait < release_rm && release_rm < web_run,
        "release run, wait and rm did not all come before the web copy: {calls:#?}"
    );
    assert_eq!(server.containers(), ["myapp-db-1", "myapp-web-1"]);
}

#[test]
fn failing_release_aborts_before_rollout() {
    let tmp = tempfile::tempdir().unwrap();
    let server = mock_server(tmp.path(), "myapp", DB_AND_WEB, ENV);
    with_release(&server);
    deploy(&server).unwrap();
    std::fs::write(server.state.join("release-exits"), "").unwrap();

    let err = deploy(&server).unwrap_err();

    let message = format!("{err:#}");
    assert!(message.contains("exited with code 1"), "{message}");
    let web_runs = server
        .calls()
        .iter()
        .filter(|l| l.starts_with("run") && l.contains("myapp-web"))
        .count();
    assert_eq!(
        web_runs, 1,
        "a new web copy started after the failed release"
    );
    assert_eq!(server.containers(), ["myapp-db-1", "myapp-web-1"]);
}
