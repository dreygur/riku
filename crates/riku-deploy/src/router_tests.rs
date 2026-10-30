use super::*;

pub(super) fn env(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

#[test]
fn domains_split_on_space_and_comma() {
    let e = env(&[("RIKU_DOMAINS", "a.com, b.com  c.com")]);
    assert_eq!(domains_from_env(&e), vec!["a.com", "b.com", "c.com"]);
    assert!(domains_from_env(&env(&[])).is_empty());
}

#[test]
fn route_plan_carries_domains_policy_and_the_env_backend() {
    let e = env(&[
        ("RIKU_DOMAINS", "shop.example.com"),
        ("PORT", "3000"),
        ("RIKU_HTTPS_ONLY", "false"),
    ]);

    let plan = route_plan(&e).unwrap();

    assert_eq!(plan.domains.len(), 1);
    assert_eq!(plan.https, HttpsPolicy::AlsoPlainHttp);
    assert_eq!(plan.backends, vec![Backend::loopback(3000)]);
}

#[test]
fn route_plan_rejects_bad_domain() {
    let err = route_plan(&env(&[("RIKU_DOMAINS", "a.com {")])).unwrap_err();

    assert!(err.to_string().contains("a.com {"), "{err}");
}

#[test]
fn upstream_port_prefers_internal_then_port_then_default() {
    assert_eq!(
        upstream_port_from_env(&env(&[("RIKU_UPSTREAM_PORT", "9000"), ("PORT", "3000")])),
        9000
    );
    assert_eq!(upstream_port_from_env(&env(&[("PORT", "3000")])), 3000);
    assert_eq!(upstream_port_from_env(&env(&[])), 8080);
    // Garbage falls back to the default rather than panicking.
    assert_eq!(upstream_port_from_env(&env(&[("PORT", "nope")])), 8080);
}
