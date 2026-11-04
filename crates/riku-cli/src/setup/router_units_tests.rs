use std::path::PathBuf;

use super::*;

#[test]
fn nginx_path_unit_watches_its_config_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let nginx_dir = PathBuf::from("/home/deploy/.riku/nginx");

    let enabled = write_router_units(tmp.path(), &Router::Nginx, &nginx_dir).unwrap();
    assert_eq!(enabled.as_deref(), Some("riku-nginx.path"));

    let path_unit = fs::read_to_string(tmp.path().join("riku-nginx.path")).unwrap();
    assert!(
        path_unit.contains("PathModified=/home/deploy/.riku/nginx"),
        "{path_unit}"
    );
    assert!(
        path_unit.contains("Unit=riku-nginx-reload.service"),
        "{path_unit}"
    );

    let reload = fs::read_to_string(tmp.path().join("riku-nginx-reload.service")).unwrap();
    assert!(
        reload.contains("ExecStart=/usr/bin/systemctl reload nginx"),
        "{reload}"
    );
}

#[test]
fn caddy_and_plugin_routers_get_no_units() {
    let tmp = tempfile::tempdir().unwrap();
    for router in [Router::Caddy, Router::Plugin("traefik".to_string())] {
        let enabled = write_router_units(tmp.path(), &router, tmp.path()).unwrap();
        assert_eq!(enabled, None);
    }
    assert_eq!(
        fs::read_dir(tmp.path()).unwrap().count(),
        0,
        "no unit files expected"
    );
    assert_eq!(
        unit_ordering(&Router::Plugin("traefik".to_string())),
        "After=network.target"
    );
}
