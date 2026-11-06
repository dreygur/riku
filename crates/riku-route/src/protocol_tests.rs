use super::*;

const ROUTE: &str =
    r#"{"domains":["myapp.example"],"https":"also_plain_http","backends":[{"unix_copy":1}]}"#;

#[test]
fn apply_survives_encode_and_parse() {
    let route: Route = serde_json::from_str(ROUTE).unwrap();
    let request = Request::Apply {
        app: "myapp".to_string(),
        router: RouterKind::Caddy,
        route: Box::new(route),
    };

    let bytes = encode_request(&request).unwrap();

    assert_eq!(parse_request(&bytes).unwrap(), request);
}

#[test]
fn unknown_field_is_refused_at_every_level() {
    let top = format!(r#"{{"v":1,"op":"apply","app":"myapp","route":{ROUTE},"path":"/etc/x"}}"#);
    let variant = r#"{"v":1,"op":"remove","app":"myapp","path":"/etc/x"}"#;
    let status = r#"{"v":1,"op":"status","verbose":true}"#;
    let route = format!(
        r#"{{"v":1,"op":"apply","app":"myapp","route":{}}}"#,
        ROUTE.replace(r#""https""#, r#""root":"/","https""#)
    );

    for body in [top.as_str(), variant, status, route.as_str()] {
        assert!(
            parse_request(body.as_bytes()).is_err(),
            "accepted a request with an unknown field: {body}"
        );
    }
}

#[test]
fn other_versions_and_missing_versions_are_refused() {
    for body in [
        r#"{"v":2,"op":"status"}"#,
        r#"{"v":0,"op":"status"}"#,
        r#"{"op":"status"}"#,
    ] {
        assert!(parse_request(body.as_bytes()).is_err(), "accepted {body}");
    }
}

#[test]
fn replies_are_tagged_and_survive_a_round_trip() {
    let ok = encode_reply(&Reply::Ok { reloaded: true }).unwrap();
    let failed = Reply::Failed {
        stage: Stage::Check,
        error: "bad".to_string(),
    };

    assert_eq!(ok, br#"{"reply":"ok","reloaded":true}"#);
    assert_eq!(
        parse_reply(&encode_reply(&failed).unwrap()).unwrap(),
        failed
    );
    assert!(parse_reply(br#"{"reply":"maybe"}"#).is_err());
}
