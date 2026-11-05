use super::*;

fn parse(json: &str) -> Result<Route, serde_json::Error> {
    serde_json::from_str(json)
}

#[test]
fn minimal_route_takes_the_listen_defaults() {
    let route = parse(
        r#"{"domains":["myapp.example"],"https":"also_plain_http","backends":[{"tcp":{"host":"127.0.0.1","port":5000}},{"unix_copy":2}]}"#,
    )
    .unwrap();

    assert_eq!(route.https, HttpsPolicy::AlsoPlainHttp);
    assert_eq!(route.listen, Listen::default());
    assert_eq!(route.backends[1], RouteBackend::UnixCopy(2));
    assert!(route.tls.is_none());
}

#[test]
fn tls_key_stays_out_of_debug_output() {
    let tls = Tls {
        cert_pem: "cert".to_string(),
        key_pem: "secret-key-text".to_string(),
    };

    assert!(!format!("{tls:?}").contains("secret-key-text"));
}
