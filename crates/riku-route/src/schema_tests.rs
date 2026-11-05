use super::*;

fn parse(json: &str) -> Result<Route, serde_json::Error> {
    serde_json::from_str(json)
}

#[test]
fn unknown_field_is_refused() {
    let top = parse(
        r#"{"domains":["myapp.example"],"https":"only","backends":[{"unix_copy":1}],"proxy_pass":"x"}"#,
    );
    assert!(top.is_err(), "an unknown top level field was accepted");

    let nested = parse(
        r#"{"domains":["myapp.example"],"https":"only","backends":[{"unix_copy":1}],"listen":{"port":80,"root":"/etc"}}"#,
    );
    assert!(
        nested.is_err(),
        "an unknown field inside listen was accepted"
    );

    let tls = parse(
        r#"{"domains":["myapp.example"],"https":"only","backends":[{"unix_copy":1}],"tls":{"cert_pem":"a","key_pem":"b","path":"/etc/x"}}"#,
    );
    assert!(tls.is_err(), "an unknown field inside tls was accepted");

    let cache = parse(
        r#"{"domains":["myapp.example"],"https":"only","backends":["unix_default"],"cache":{"prefixes":[],"time_s":1,"size_gb":1,"expiry_s":1,"redirects_s":1,"any_s":1,"control_s":1,"path":"/x"}}"#,
    );
    assert!(cache.is_err(), "an unknown field inside cache was accepted");

    let backend = parse(
        r#"{"domains":["myapp.example"],"https":"only","backends":[{"tcp":{"host":"127.0.0.1","port":5000,"path":"/x"}}]}"#,
    );
    assert!(
        backend.is_err(),
        "an unknown field inside a backend was accepted"
    );
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
    assert!(route.tls.is_none() && route.snippet.is_none());
}

#[test]
fn tls_key_stays_out_of_debug_output() {
    let tls = Tls {
        cert_pem: "cert".to_string(),
        key_pem: "secret-key-text".to_string(),
    };

    assert!(!format!("{tls:?}").contains("secret-key-text"));
}
