use super::*;

fn env(pairs: &[(&str, &str)]) -> HashMap<String, String> {
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

#[test]
fn https_truthiness() {
    assert!(https_from_env(&env(&[("RIKU_HTTPS_ONLY", "1")])));
    assert!(https_from_env(&env(&[("RIKU_HTTPS_ONLY", "true")])));
    assert!(!https_from_env(&env(&[("RIKU_HTTPS_ONLY", "false")])));
    assert!(!https_from_env(&env(&[("RIKU_HTTPS_ONLY", "0")])));
    assert!(!https_from_env(&env(&[])));
}

fn caddy_err(pairs: &[(&str, &str)]) -> String {
    match caddy_request(&env(pairs)) {
        Ok(_) => panic!("{pairs:?} was accepted"),
        Err(e) => e.to_string(),
    }
}

#[test]
fn rejects_bad_domain() {
    let err = caddy_err(&[("RIKU_DOMAINS", "a.com {")]);

    assert!(
        err.contains("a.com {"),
        "error should name the value: {err}"
    );
}

#[test]
fn none_without_domains() {
    assert!(caddy_request(&env(&[])).unwrap().is_none());
    assert!(caddy_request(&env(&[("RIKU_DOMAINS", " ")]))
        .unwrap()
        .is_none());
}

#[test]
fn rejects_socket_apps() {
    for key in ["RIKU_NGINX_WSGI", "RIKU_NGINX_STATIC"] {
        let err = caddy_err(&[("RIKU_DOMAINS", "a.com"), (key, "true")]);
        assert!(err.contains("Caddy proxies TCP only"), "{key}: {err}");
    }
}
