use riku_route::Route;

use super::*;

fn minimal_route() -> Route {
    serde_json::from_str(
        r#"{"domains":["myapp.example"],"https":"also_plain_http","backends":["unix_default"]}"#,
    )
    .unwrap()
}

fn assert_failed_at(reply: &Reply, expected: Stage) {
    match reply {
        Reply::Failed { stage, .. } => assert_eq!(*stage, expected, "{reply:?}"),
        other => panic!("expected a failure at {expected:?}, got {other:?}"),
    }
}

#[test]
fn non_root_refuses_to_start() {
    let error = not_root(Uid::from_raw(1000)).expect("uid 1000 was allowed to start");

    assert!(error.to_string().contains("must run as root"), "{error}");
    assert!(not_root(Uid::from_raw(0)).is_none());
}

#[test]
fn bad_app_name_is_a_schema_failure() {
    let reply = handle(Request::Remove {
        app: "../etc".to_string(),
    });

    assert_failed_at(&reply, Stage::Schema);
}

#[test]
fn remove_of_a_valid_app_fails_at_render_until_implemented() {
    let reply = handle(Request::Remove {
        app: "myapp".to_string(),
    });

    assert_failed_at(&reply, Stage::Render);
}

#[test]
fn apply_of_a_valid_route_fails_at_render_until_implemented() {
    let reply = handle(Request::Apply {
        app: "myapp".to_string(),
        route: Box::new(minimal_route()),
    });

    assert_failed_at(&reply, Stage::Render);
}
