use super::*;

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
fn caddy_proxies_to_loopback_and_others_keep_the_copy_address() {
    let host = "10.0.0.5".parse().unwrap();
    let copies = vec![
        Backend::Tcp { host, port: 5001 },
        Backend::Unix("/srv/a.sock".into()),
    ];
    let plugin = Router::Plugin("edge-router".to_string());

    assert_eq!(
        backends_for(&Router::Caddy, copies.clone()),
        vec![Backend::loopback(5001), Backend::Unix("/srv/a.sock".into())]
    );
    assert_eq!(backends_for(&plugin, copies.clone()), copies);
    assert_eq!(backends_for(&Router::Nginx, copies.clone()), copies);
}
