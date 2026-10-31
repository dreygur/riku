use super::*;

const LOOPBACK: IpAddr = IpAddr::V4(std::net::Ipv4Addr::LOCALHOST);

pub(super) fn env(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

#[test]
fn route_plan_carries_domains_policy_and_the_given_backends() {
    let e = env(&[
        ("RIKU_DOMAINS", "shop.example.com"),
        ("RIKU_HTTPS_ONLY", "false"),
    ]);

    let plan = route_plan(&e, &[Backend::loopback(3000)]).unwrap();

    assert_eq!(plan.domains.len(), 1);
    assert_eq!(plan.https, HttpsPolicy::AlsoPlainHttp);
    assert_eq!(plan.backends, vec![Backend::loopback(3000)]);
}

#[test]
fn route_plan_rejects_bad_domain() {
    let err = route_plan(&env(&[("RIKU_DOMAINS", "a.com {")]), &[]).unwrap_err();

    assert!(err.to_string().contains("a.com {"), "{err}");
}

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
        initial_backends(&Router::Caddy, "shop", &e, &paths).unwrap(),
        vec![Backend::loopback(5001)]
    );
    assert_eq!(
        initial_backends(&plugin, "shop", &e, &paths).unwrap(),
        vec![Backend::Tcp {
            host: "10.0.0.5".parse().unwrap(),
            port: 5001
        }]
    );
    let err = initial_backends(
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
