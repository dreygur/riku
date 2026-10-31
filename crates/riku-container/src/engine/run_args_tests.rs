use std::path::{Path, PathBuf};

use super::run_args;
use crate::engine::fixture::sample_spec;
use crate::engine::{PullPolicy, RunSpec};
use crate::types::{HostIp, Mount, MountSource, PortBinding, Protocol, StopSignal};

pub(super) fn argv(spec: &RunSpec) -> Vec<String> {
    run_args(spec, Path::new("/scratch/env-1"))
        .unwrap()
        .into_iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect()
}

#[test]
fn pull_never_adds_the_flag_and_the_default_does_not() {
    let mut spec = sample_spec();
    assert!(!argv(&spec).iter().any(|a| a.starts_with("--pull")));

    spec.pull = PullPolicy::Never;

    assert!(argv(&spec).contains(&"--pull=never".to_string()));
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
        Mount {
            source: MountSource::Anonymous,
            target: PathBuf::from("/seed"),
            read_only: true,
        },
    ];

    let args = argv(&spec).join(" ");

    for flag in [
        "-v shop_data:/var/lib/data",
        "-v /srv/conf:/etc/conf:ro",
        "-v /cache ",
        "--mount type=volume,dst=/seed,readonly ",
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

#[test]
fn debug_output_lists_env_names_but_never_values() {
    let mut spec = sample_spec();
    spec.env
        .insert("DATABASE_URL".into(), "postgres://u:secret@db/x".into());

    let shown = format!("{spec:?}");

    assert!(
        shown.contains("DATABASE_URL"),
        "names should be listed: {shown}"
    );
    assert!(!shown.contains("secret"), "Debug leaked a value: {shown}");
}
