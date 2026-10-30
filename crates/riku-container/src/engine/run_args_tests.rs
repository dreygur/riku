use std::path::{Path, PathBuf};
use std::time::Duration;

use super::run_args;
use crate::engine::fixture::sample_spec;
use crate::engine::RunSpec;
use crate::types::{
    HealthCheck, HealthTest, HostIp, Mount, MountSource, PortBinding, Protocol, StopSignal,
};

fn argv(spec: &RunSpec) -> Vec<String> {
    run_args(spec, Path::new("/scratch/env-1"))
        .into_iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect()
}

fn port(host_ip: HostIp, host_port: Option<u16>) -> PortBinding {
    PortBinding {
        host_ip,
        host_port,
        container_port: 5432,
        protocol: Protocol::Tcp,
    }
}

#[test]
fn leading_flags_keep_the_engine_restart_policy_off() {
    let args = argv(&sample_spec()).join(" ");

    assert!(
        args.starts_with(
            "run -d --name myapp-web-1 --restart no --network riku-myapp --network-alias web \
             --label riku.app=myapp --label riku.copy=1 --label riku.service=web \
             --env-file /scratch/env-1 --stop-signal SIGTERM --stop-timeout 30 registry.example/myapp:1"
        ),
        "{args}"
    );
}

#[test]
fn published_ports_bind_loopback_unless_public() {
    let mut spec = sample_spec();
    spec.ports = vec![
        port(HostIp::Loopback, Some(5432)),
        port(HostIp::All, Some(5433)),
        port(HostIp::Loopback, None),
    ];

    let args = argv(&spec).join(" ");

    assert!(args.contains("-p 127.0.0.1:5432:5432/tcp"), "{args}");
    assert!(args.contains("-p 5433:5432/tcp"), "{args}");
    assert!(args.contains("-p 127.0.0.1::5432/tcp"), "{args}");
}

#[test]
fn mounts_render_volume_bind_anonymous_and_read_only() {
    let mut spec = sample_spec();
    spec.mounts = vec![
        Mount {
            source: MountSource::Volume("shop_data".into()),
            target: PathBuf::from("/var/lib/data"),
            read_only: false,
        },
        Mount {
            source: MountSource::Bind(PathBuf::from("/srv/conf")),
            target: PathBuf::from("/etc/conf"),
            read_only: true,
        },
        Mount {
            source: MountSource::Anonymous,
            target: PathBuf::from("/cache"),
            read_only: false,
        },
    ];

    let args = argv(&spec).join(" ");

    for flag in [
        "-v shop_data:/var/lib/data",
        "-v /srv/conf:/etc/conf:ro",
        "-v /cache",
    ] {
        assert!(args.contains(flag), "{flag} missing from {args}");
    }
}

#[test]
fn entrypoint_list_splits_into_flag_and_leading_args() {
    let mut spec = sample_spec();
    spec.entrypoint = Some(vec!["python".into(), "-u".into()]);
    spec.command = Some(vec!["app.py".into()]);

    let args = argv(&spec).join(" ");

    assert!(
        args.ends_with("--entrypoint python registry.example/myapp:1 -u app.py"),
        "{args}"
    );
}

#[test]
fn health_check_renders_shell_exec_and_disabled_forms() {
    let mut spec = sample_spec();
    spec.healthcheck = Some(HealthCheck {
        test: HealthTest::Exec(vec!["curl".into(), "-f".into(), "http://x/a b".into()]),
        interval: Some(Duration::from_secs(10)),
        timeout: None,
        retries: Some(3),
        start_period: Some(Duration::from_secs(5)),
    });
    let args = argv(&spec).join(" ");
    assert!(
        args.contains("--health-cmd curl -f 'http://x/a b'"),
        "{args}"
    );
    assert!(args.contains("--health-interval 10s"), "{args}");
    assert!(args.contains("--health-retries 3"), "{args}");
    assert!(args.contains("--health-start-period 5s"), "{args}");

    spec.healthcheck = Some(HealthCheck {
        test: HealthTest::Disabled,
        interval: None,
        timeout: None,
        retries: None,
        start_period: None,
    });
    assert!(argv(&spec).contains(&"--no-healthcheck".to_string()));
}

#[test]
fn secrets_never_appear_in_argv() {
    let mut spec = sample_spec();
    spec.env
        .insert("DATABASE_URL".into(), "postgres://u:secret@db/x".into());
    spec.stop_signal = StopSignal::try_from("sigint").unwrap();

    let args = argv(&spec);

    assert!(
        args.iter().all(|a| !a.contains("secret")),
        "secret leaked into {args:?}"
    );
    assert!(args.iter().all(|a| !a.contains("DATABASE_URL")), "{args:?}");
    assert!(args.contains(&"--env-file".to_string()), "{args:?}");
}
