use super::*;

const LOOPBACK: IpAddr = IpAddr::V4(std::net::Ipv4Addr::LOCALHOST);

pub(super) fn env(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

#[test]
fn route_plan_carries_domains_policy_and_the_env_backend() {
    let e = env(&[
        ("RIKU_DOMAINS", "shop.example.com"),
        ("PORT", "3000"),
        ("RIKU_HTTPS_ONLY", "false"),
    ]);

    let plan = route_plan(&e, LOOPBACK).unwrap();

    assert_eq!(plan.domains.len(), 1);
    assert_eq!(plan.https, HttpsPolicy::AlsoPlainHttp);
    assert_eq!(plan.backends, vec![Backend::loopback(3000)]);
}

#[test]
fn route_plan_rejects_bad_domain() {
    let err = route_plan(&env(&[("RIKU_DOMAINS", "a.com {")]), LOOPBACK).unwrap_err();

    assert!(err.to_string().contains("a.com {"), "{err}");
}

#[test]
fn upstream_port_prefers_internal_then_port_then_default() {
    let port_of = |pairs: &[(&str, &str)]| route_plan(&env(pairs), LOOPBACK).unwrap().backends;

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
        let err = match route_plan(&env(&[(key, value)]), LOOPBACK) {
            Ok(plan) => panic!("{key}={value} was routed to {:?}", plan.backends),
            Err(e) => e.to_string(),
        };
        assert!(err.contains(key) && err.contains(value), "{err}");
    }
}
