use super::*;
use crate::router::app_routing::RoutingMode;

fn routing(domains: &[&str], port: Option<u16>, mode: RoutingMode) -> AppRouting {
    AppRouting {
        domains: domains.iter().map(|d| d.to_string()).collect(),
        fallback: false,
        port,
        mode,
        legacy: false,
        fallback_skipped: None,
    }
}

#[test]
fn a_path_mode_app_has_no_site_of_its_own() {
    let shop = routing(&[], None, RoutingMode::Path);

    assert!(!site_for(&shop, &Router::Nginx));
    assert!(!site_for(&shop, &Router::Caddy));
}

#[test]
fn caddy_keeps_no_site_for_an_app_with_no_domain_and_no_port() {
    assert!(!site_for(
        &routing(&[], None, RoutingMode::Domain),
        &Router::Caddy
    ));
    assert!(site_for(
        &routing(&[], Some(8100), RoutingMode::Port),
        &Router::Caddy
    ));
    assert!(site_for(
        &routing(&["a.example"], None, RoutingMode::Domain),
        &Router::Caddy
    ));
}

fn app_running(paths: &RikuPaths, kind: &str) {
    std::fs::create_dir_all(&paths.workers_enabled).unwrap();
    std::fs::create_dir_all(paths.app_root.join("shop")).unwrap();
    let config = crate::supervisor::config::create_worker_config(
        "shop",
        kind,
        "run",
        1,
        std::collections::HashMap::new(),
        "/srv/app",
        "/srv/log",
    );
    std::fs::write(
        paths.workers_enabled.join(format!("shop@{kind}.1.toml")),
        toml::to_string(&config).unwrap(),
    )
    .unwrap();
}

/// A worker only app gets no route since the deploy takes it off the
/// router, so `doctor --fix` must not redeploy it for a missing site. An
/// app with a web copy still expects one.
#[test]
fn a_worker_only_app_expects_no_site_and_a_web_app_does() {
    let tmp = tempfile::tempdir().unwrap();
    let worker_only = RikuPaths::for_tests(&tmp.path().join("a"));
    app_running(&worker_only, "worker");
    let web = RikuPaths::for_tests(&tmp.path().join("b"));
    app_running(&web, "web");

    assert!(!site_expected("shop", &worker_only).unwrap());
    assert!(site_expected("shop", &web).unwrap());
}

#[test]
fn nginx_keeps_a_site_for_an_app_with_no_domain() {
    assert!(site_for(
        &routing(&[], None, RoutingMode::Domain),
        &Router::Nginx
    ));
}
