use super::*;
use crate::types::HealthTest;
use serde_json::json;
use std::time::Duration;

#[test]
fn exec_shell_and_disabled_forms() {
    let exec = parse_healthcheck(&json!({
        "test": ["CMD", "curl", "-f", "http://localhost/healthz"],
        "interval": "10s",
        "timeout": "1m30s",
        "start_period": "5s",
        "retries": 3,
    }))
    .unwrap();
    assert_eq!(
        exec,
        HealthCheck {
            test: HealthTest::Exec(vec![
                "curl".into(),
                "-f".into(),
                "http://localhost/healthz".into()
            ]),
            interval: Some(Duration::from_secs(10)),
            timeout: Some(Duration::from_secs(90)),
            retries: Some(3),
            start_period: Some(Duration::from_secs(5)),
        }
    );

    let shell = parse_healthcheck(&json!({"test": "pg_isready"})).unwrap();
    assert_eq!(shell.test, HealthTest::Shell("pg_isready".into()));
    assert_eq!(shell.interval, None);

    let cmd_shell = parse_healthcheck(&json!({"test": ["CMD-SHELL", "pg_isready -q"]})).unwrap();
    assert_eq!(cmd_shell.test, HealthTest::Shell("pg_isready -q".into()));

    for value in [
        json!({"disable": true}),
        json!({"test": ["NONE"]}),
        json!({"disable": true, "test": "ignored", "retries": 2}),
    ] {
        assert_eq!(
            parse_healthcheck(&value).unwrap().test,
            HealthTest::Disabled,
            "{value}"
        );
    }
}

#[test]
fn refuses_unknown_keys_bad_forms_and_numeric_durations() {
    for value in [
        json!({"test": "x", "start_interval": "1s"}),
        json!({"test": ["curl", "-f", "x"]}),
        json!({"test": ["CMD"]}),
        json!({"test": ["CMD-SHELL", "a", "b"]}),
        json!({"retries": 3}),
        json!({"test": "x", "interval": 10}),
        json!({"test": "x", "retries": -1}),
        json!({"test": "x", "retries": "3"}),
        json!({"test": "x", "timeout": "soon"}),
    ] {
        assert!(parse_healthcheck(&value).is_err(), "{value} was accepted");
    }
}
