use std::cell::RefCell;
use std::net::Ipv4Addr;

use super::*;
use crate::router::app_routing::RoutingMode;
use crate::util::route_plan::HttpsPolicy;

fn fallback(domain: &str, private_ip: Option<Ipv4Addr>) -> AppRouting {
    AppRouting {
        domains: vec![domain.to_string()],
        fallback: true,
        port: None,
        mode: RoutingMode::Domain,
        fallback_skipped: None,
        fallback_private_ip: private_ip,
    }
}

/// The https policy of the route `apply_caddy_site` sent for `routing`.
fn sent_https(routing: &AppRouting) -> HttpsPolicy {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    let seen = RefCell::new(Vec::new());
    let send = |request: &Request| {
        seen.borrow_mut().push(request.clone());
        Ok(Reply::Ok { reloaded: true })
    };
    let env = HashMap::from([("RIKU_DOMAINS".to_string(), routing.domains.join(","))]);

    apply_caddy_site(
        send,
        "myapp",
        &env,
        routing,
        &[Backend::loopback(5000)],
        &paths,
    )
    .unwrap();

    let requests = seen.take();
    match requests.as_slice() {
        [Request::Apply { route, .. }] => route.https,
        other => panic!("expected one apply, got {other:?}"),
    }
}

#[test]
fn fallback_on_a_private_address_gets_a_plain_http_only_site() {
    let routing = fallback("myapp.10.0.0.5.example", Some(Ipv4Addr::new(10, 0, 0, 5)));

    assert_eq!(sent_https(&routing), HttpsPolicy::PlainHttpOnly);
}

#[test]
fn fallback_on_a_public_address_keeps_caddys_automatic_https() {
    let routing = fallback("myapp.203.0.113.10.example", None);

    assert_eq!(sent_https(&routing), HttpsPolicy::AlsoPlainHttp);
}
