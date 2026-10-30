use std::path::PathBuf;

use super::*;

#[test]
fn path_unit_watches_caddy() {
    let tmp = tempfile::tempdir().unwrap();
    let caddy_dir = PathBuf::from("/home/deploy/.riku/caddy");

    let enabled = write_router_units(tmp.path(), &Router::Caddy, &caddy_dir).unwrap();
    assert_eq!(enabled.as_deref(), Some("riku-caddy.path"));

    let path_unit = fs::read_to_string(tmp.path().join("riku-caddy.path")).unwrap();
    assert!(
        path_unit.contains("PathModified=/home/deploy/.riku/caddy"),
        "{path_unit}"
    );
    assert!(
        path_unit.contains("Unit=riku-caddy-reload.service"),
        "{path_unit}"
    );

    let reload = fs::read_to_string(tmp.path().join("riku-caddy-reload.service")).unwrap();
    assert!(
        reload.contains("ExecStart=/usr/bin/systemctl reload caddy"),
        "{reload}"
    );
}

#[test]
fn plugin_router_gets_no_units() {
    let tmp = tempfile::tempdir().unwrap();
    let router = Router::Plugin("traefik".to_string());

    let enabled = write_router_units(tmp.path(), &router, tmp.path()).unwrap();
    assert_eq!(enabled, None);
    assert_eq!(
        fs::read_dir(tmp.path()).unwrap().count(),
        0,
        "no unit files expected"
    );
    assert_eq!(unit_ordering(&router), "After=network.target");
}
