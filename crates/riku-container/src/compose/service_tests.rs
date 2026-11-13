use crate::compose::test_support::{service_error as error_of, ServiceFixture as Fixture};
use crate::compose::{DependsCondition, Value};
use crate::types::{HostIp, PortBinding, Protocol, RestartPolicy};
use serde_json::json;
use std::time::Duration;

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
    assert_eq!(svc.stop_grace, Duration::from_secs(10));
    assert_eq!(svc.restart, RestartPolicy::No);
    assert!(
        svc.init,
        "a service runs with the engine's init unless it says no"
    );
}

#[test]
fn init_false_turns_the_engine_init_off_and_a_non_bool_is_refused() {
    let off = Fixture::new()
        .parse("web", json!({"image": "myapp", "init": false}))
        .unwrap();
    assert!(!off.init);

    let err = error_of(Fixture::new().parse("web", json!({"image": "myapp", "init": "yes"})));
    assert!(err.contains("init"), "{err}");
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
fn extension_keys_on_a_service_are_ignored() {
    let svc = Fixture::new()
        .parse(
            "web",
            json!({"image": "myapp", "x-notes": {"owner": "web team"}, "x-tier": 1}),
        )
        .unwrap();

    assert_eq!(svc.image.as_deref(), Some("myapp"));
}

#[test]
fn compose_refuses_riku_port_label() {
    let fx = Fixture::new();
    for label in ["riku.port", "riku.restart"] {
        let err = error_of(fx.parse("web", json!({"image": "myapp", "labels": {label: "1"}})));
        assert!(err.contains(label), "{label}: {err}");
    }
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
