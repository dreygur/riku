use super::*;
use crate::router::tests::env;

const LOOPBACK: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);

#[test]
fn upstream_port_prefers_internal_then_port_then_default() {
    let port_of = |pairs: &[(&str, &str)]| tcp_backend(&env(pairs), LOOPBACK).unwrap();

    assert_eq!(
        port_of(&[("RIKU_UPSTREAM_PORT", "9000"), ("PORT", "3000")]),
        vec![Backend::loopback(9000)]
    );
    assert_eq!(port_of(&[("PORT", "3000")]), vec![Backend::loopback(3000)]);
    assert_eq!(port_of(&[]), vec![Backend::loopback(8080)]);
}

#[test]
fn unparsable_port_is_refused_naming_the_key() {
    for (key, value) in [("PORT", "nope"), ("RIKU_UPSTREAM_PORT", "80x")] {
        let err = match tcp_backend(&env(&[(key, value)]), LOOPBACK) {
            Ok(backends) => panic!("{key}={value} was routed to {backends:?}"),
            Err(e) => e.to_string(),
        };
        assert!(err.contains(key) && err.contains(value), "{err}");
    }
}

#[test]
fn caddy_proxies_to_loopback_and_plugins_to_the_bind_address() {
    let paths = RikuPaths::for_tests(std::path::Path::new("/nonexistent"));
    let e = env(&[("PORT", "5001"), ("RIKU_BIND_ADDRESS", "10.0.0.5")]);
    let plugin = Router::Plugin("edge-router".to_string());

    assert_eq!(
        fallback_backends(&Router::Caddy, "shop", &e, &paths).unwrap(),
        vec![Backend::loopback(5001)]
    );
    assert_eq!(
        fallback_backends(&plugin, "shop", &e, &paths).unwrap(),
        vec![Backend::Tcp {
            host: "10.0.0.5".parse().unwrap(),
            port: 5001
        }]
    );
    let err = fallback_backends(
        &plugin,
        "shop",
        &env(&[("RIKU_BIND_ADDRESS", "localhost")]),
        &paths,
    )
    .unwrap_err()
    .to_string();
    assert!(
        err.contains("RIKU_BIND_ADDRESS") && err.contains("localhost"),
        "{err}"
    );
}
