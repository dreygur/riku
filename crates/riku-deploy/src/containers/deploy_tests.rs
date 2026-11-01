use crate::containers::mock_state::{deploy, mock_server, DB_AND_WEB};

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
