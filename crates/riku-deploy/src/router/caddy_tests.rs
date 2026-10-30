use super::*;
use crate::router::tests::env;

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
