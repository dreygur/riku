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

#[test]
fn nginx_keeps_a_site_for_an_app_with_no_domain() {
    assert!(site_for(
        &routing(&[], None, RoutingMode::Domain),
        &Router::Nginx
    ));
}
