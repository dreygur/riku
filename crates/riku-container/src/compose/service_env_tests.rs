use crate::compose::test_support::{service_error as error_of, ServiceFixture as Fixture};
use crate::compose::{ServiceContext, Value};
use serde_json::json;
use std::collections::BTreeMap;

#[test]
fn environment_wins_over_env_file_and_bare_keys_use_the_variables() {
    let mut fx = Fixture::new();
    fx.write("app.env", "KEY=a\nONLY_FILE=f\n");
    fx.vars.insert("FROM_VARS".into(), "v".into());
    let svc = fx
        .parse(
            "web",
            json!({
                "image": "myapp",
                "env_file": "app.env",
                "environment": ["KEY=b", "FROM_VARS", "UNSET_ONE", "COUNT=3"],
            }),
        )
        .unwrap();
    let expected: BTreeMap<String, String> = [
        ("KEY", "b"),
        ("ONLY_FILE", "f"),
        ("FROM_VARS", "v"),
        ("COUNT", "3"),
    ]
    .iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect();
    assert_eq!(svc.environment, expected);
}

#[test]
fn environment_map_skips_null_and_refuses_floats() {
    let fx = Fixture::new();
    let svc = fx
        .parse(
            "web",
            json!({"image": "myapp", "environment": {"A": null, "B": true, "C": 7}}),
        )
        .unwrap();
    assert_eq!(svc.environment.len(), 2);
    assert_eq!(svc.environment["B"], "true");
    assert_eq!(svc.environment["C"], "7");

    let err = error_of(fx.parse(
        "web",
        json!({"image": "myapp", "environment": {"PY_VERSION": 3.10}}),
    ));
    assert!(
        err.contains("service 'web'") && err.contains("PY_VERSION") && err.contains("quote"),
        "{err}"
    );
}

#[test]
fn environment_value_with_a_line_break_is_refused_naming_the_key() {
    let fx = Fixture::new();
    for environment in [json!(["TOKEN=x\ny"]), json!({"TOKEN": "x\ny"})] {
        let err = error_of(fx.parse("web", json!({"image": "myapp", "environment": environment})));
        assert!(
            err.contains("service 'web'") && err.contains("TOKEN") && !err.contains("x\ny"),
            "{err}"
        );
    }
}

#[test]
fn leading_zero_numbers_are_refused_because_yaml_reads_them_as_floats() {
    let doc: Value = serde_saphyr::from_str("image: myapp\nenvironment:\n  UMASK: 022\n").unwrap();
    let err = error_of(Fixture::new().parse("web", doc));
    assert!(
        err.contains("service 'web'") && err.contains("UMASK") && err.contains("quote"),
        "{err}"
    );
}

#[test]
fn context_debug_output_omits_variable_values() {
    let fx = Fixture::new();
    let mut vars = BTreeMap::new();
    vars.insert("DB_PASSWORD".to_string(), "hunter2".to_string());
    let ctx = ServiceContext {
        app_path: fx.app.path(),
        compose_dir: fx.app.path(),
        vars: &vars,
        declared_volumes: &fx.volumes,
    };
    let shown = format!("{ctx:?}");
    assert!(
        shown.contains("DB_PASSWORD") && !shown.contains("hunter2"),
        "{shown}"
    );
}

#[test]
fn env_file_outside_app_is_refused() {
    let err = error_of(Fixture::new().parse(
        "web",
        json!({"image": "myapp", "env_file": "../../envs/shop/ENV"}),
    ));
    assert!(
        err.contains("service 'web'") && err.contains("envs/shop/ENV"),
        "{err}"
    );
}
