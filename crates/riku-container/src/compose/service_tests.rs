use super::*;
use crate::compose::{DependsCondition, ServiceContext, ServiceDef};
use crate::types::{HostIp, PortBinding, Protocol};
use anyhow::Result;
use serde_json::json;
use std::collections::BTreeSet;
use std::time::Duration;
use tempfile::TempDir;

struct Fixture {
    app: TempDir,
    vars: BTreeMap<String, String>,
    volumes: BTreeSet<String>,
}

impl Fixture {
    fn new() -> Fixture {
        Fixture {
            app: TempDir::new().unwrap(),
            vars: BTreeMap::new(),
            volumes: ["pgdata".to_string()].into(),
        }
    }

    fn parse(&self, name: &str, value: Value) -> Result<ServiceDef> {
        let ctx = ServiceContext {
            app_path: self.app.path(),
            compose_dir: self.app.path(),
            vars: &self.vars,
            declared_volumes: &self.volumes,
        };
        parse_service(&ctx, name, &value)
    }

    fn write(&self, relative: &str, text: &str) {
        std::fs::write(self.app.path().join(relative), text).unwrap();
    }
}

fn error_of(result: Result<ServiceDef>) -> String {
    format!("{:#}", result.expect_err("parse succeeded"))
}

#[test]
fn database_port_binds_loopback_by_default() {
    let svc = Fixture::new()
        .parse(
            "db",
            json!({"image": "postgres:16", "ports": ["5432:5432"]}),
        )
        .unwrap();
    assert_eq!(
        svc.ports,
        vec![PortBinding {
            host_ip: HostIp::Loopback,
            host_port: Some(5432),
            container_port: 5432,
            protocol: Protocol::Tcp,
        }]
    );
    assert_eq!(svc.stop_grace, Duration::from_secs(30));
    assert_eq!(svc.restart, RestartPolicy::No);
}

#[test]
fn public_label_exposes_on_all_interfaces() {
    let svc = Fixture::new()
        .parse(
            "proxy",
            json!({"image": "caddy", "ports": ["80:80"], "labels": {"riku.public": "true"}}),
        )
        .unwrap();
    assert_eq!(svc.ports[0].host_ip, HostIp::All);
    assert_eq!(svc.labels["riku.public"], "true");
}

#[test]
fn public_label_with_other_value_is_refused() {
    let err = error_of(Fixture::new().parse(
        "proxy",
        json!({"image": "caddy", "ports": ["80:80"], "labels": ["riku.public=yes"]}),
    ));
    assert!(
        err.contains("service 'proxy'") && err.contains("riku.public") && err.contains("yes"),
        "{err}"
    );
}

#[test]
fn explicit_host_ip_is_refused_naming_the_service() {
    let err = error_of(Fixture::new().parse(
        "proxy",
        json!({"image": "caddy", "ports": ["0.0.0.0:80:80"]}),
    ));
    assert!(
        err.contains("service 'proxy'") && err.contains("riku.public"),
        "{err}"
    );
}

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
fn build_context_may_not_pass_through_a_symlink() {
    let fx = Fixture::new();
    let outside = TempDir::new().unwrap();
    std::os::unix::fs::symlink(outside.path(), fx.app.path().join("src")).unwrap();
    std::fs::create_dir(fx.app.path().join("real")).unwrap();

    for build in [
        json!("./src"),
        json!({"context": "./src/api"}),
        json!({"context": "./real", "dockerfile": "../src/Dockerfile"}),
    ] {
        let err = error_of(fx.parse("web", json!({ "build": build })));
        assert!(
            err.contains("service 'web'") && err.contains("symlink") && err.contains("src"),
            "{build}: {err}"
        );
    }
    assert!(fx
        .parse("web", json!({"build": "./real/not-created-yet"}))
        .is_ok());
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

#[test]
fn completed_condition_is_refused() {
    let err = error_of(Fixture::new().parse(
        "web",
        json!({"image": "myapp", "depends_on": {"db": {"condition": "service_completed_successfully"}}}),
    ));
    assert!(err.contains("service_completed_successfully"), "{err}");
}

#[test]
fn depends_on_list_and_map_forms() {
    let fx = Fixture::new();
    let listed = fx
        .parse("web", json!({"image": "myapp", "depends_on": ["db"]}))
        .unwrap();
    assert_eq!(listed.depends_on[0].condition, DependsCondition::Started);
    let mapped = fx
        .parse(
            "web",
            json!({"image": "myapp", "depends_on": {"db": {"condition": "service_healthy"}}}),
        )
        .unwrap();
    assert_eq!(mapped.depends_on[0].service.as_str(), "db");
    assert_eq!(mapped.depends_on[0].condition, DependsCondition::Healthy);
}

#[test]
fn image_or_build_is_required_and_build_stays_inside_the_app() {
    let fx = Fixture::new();
    assert!(error_of(fx.parse("web", json!({}))).contains("image or build"));
    assert!(error_of(fx.parse("web", json!({"image": "-rm"}))).contains("-rm"));

    let svc = fx
        .parse(
            "web",
            json!({"build": {"context": "./api", "dockerfile": "Dockerfile.prod", "args": {"N": 2}}}),
        )
        .unwrap();
    let build = svc.build.unwrap();
    assert_eq!(build.context, fx.app.path().join("api"));
    assert_eq!(
        build.dockerfile,
        Some(fx.app.path().join("api/Dockerfile.prod"))
    );
    assert_eq!(build.args["N"], "2");

    for build in [
        json!("../elsewhere"),
        json!({"context": ".", "dockerfile": "../../x"}),
        json!({"context": ".", "target": "prod"}),
    ] {
        let err = error_of(fx.parse("web", json!({ "build": build })));
        assert!(err.contains("service 'web'"), "{err}");
    }
}

#[test]
fn command_splits_strings_with_shell_quoting() {
    let fx = Fixture::new();
    let svc = fx
        .parse(
            "worker",
            json!({"image": "myapp", "command": "run --name 'a b'", "entrypoint": ["/bin/sh", "-c"]}),
        )
        .unwrap();
    assert_eq!(svc.command.unwrap(), vec!["run", "--name", "a b"]);
    assert_eq!(svc.entrypoint.unwrap(), vec!["/bin/sh", "-c"]);

    let err = error_of(fx.parse("worker", json!({"image": "myapp", "command": "run 'oops"})));
    assert!(
        err.contains("service 'worker'") && err.contains("run 'oops"),
        "{err}"
    );
}

#[test]
fn scalar_fields_from_yaml_text() {
    let doc: Value = serde_saphyr::from_str(
        "image: myapp\nrestart: no\nstop_signal: sigint\nstop_grace_period: 1m30s\nuser: '1000:1000'\nworking_dir: /srv\nnetworks: [default]\n",
    )
    .unwrap();
    let svc = Fixture::new().parse("web", doc).unwrap();
    assert_eq!(svc.restart, RestartPolicy::No);
    assert_eq!(svc.stop_signal.as_str(), "SIGINT");
    assert_eq!(svc.stop_grace, Duration::from_secs(90));
    assert_eq!(svc.user.as_deref(), Some("1000:1000"));
    assert_eq!(svc.working_dir.as_deref(), Some("/srv"));
}

#[test]
fn restart_policies() {
    let fx = Fixture::new();
    let policy = |raw: &str| {
        fx.parse("web", json!({"image": "myapp", "restart": raw}))
            .map(|svc| svc.restart)
    };
    assert_eq!(policy("always").unwrap(), RestartPolicy::Always);
    assert_eq!(
        policy("unless-stopped").unwrap(),
        RestartPolicy::UnlessStopped
    );
    assert_eq!(
        policy("on-failure:5").unwrap(),
        RestartPolicy::OnFailure {
            max_retries: Some(5)
        }
    );
    assert!(policy("sometimes").is_err());
    assert!(policy("on-failure:x").is_err());
}

#[test]
fn refuses_custom_networks_and_unsafe_text() {
    let fx = Fixture::new();
    for value in [
        json!({"image": "myapp", "networks": ["backend"]}),
        json!({"image": "myapp", "environment": ["A\nB=1"]}),
        json!({"image": "myapp", "environment": ["=x"]}),
        json!({"image": "myapp", "labels": {"a=b": "c"}}),
        json!({"image": "myapp", "labels": {"a": "line1\nline2"}}),
        json!({"image": "myapp", "labels": {"riku.app": "other"}}),
        json!({"image": "myapp", "user": "-u"}),
        json!({"image": "myapp", "working_dir": "relative"}),
        json!({"image": "myapp", "stop_signal": "KILL;"}),
    ] {
        let err = error_of(fx.parse("web", value.clone()));
        assert!(err.contains("service 'web'"), "{value}: {err}");
    }
}
