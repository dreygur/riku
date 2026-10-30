//! The `run -d` argument list for one container.
//!
//! Security model: nothing here is passed through a shell. The environment
//! never appears in the list, only the path of the env file that carries it,
//! so secrets stay out of the process list. Callers run
//! [`super::spec_check::check_spec`] first; [`super::Engine::run`] does.

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::time::Duration;

use crate::types::{
    ContainerName, HealthCheck, HealthTest, HostIp, Mount, MountSource, NetworkName, PortBinding,
    Protocol, ServiceName, StopSignal,
};

/// Everything needed to start one container.
///
/// An `Exec` health check is joined into a shell line because the engine CLI
/// has no exec form, so an image without `/bin/sh` reports unhealthy.
#[derive(Debug, Clone)]
pub struct RunSpec {
    pub name: ContainerName,
    /// Image reference, checked by `Engine::run` before use.
    pub image: String,
    pub network: NetworkName,
    /// Name other containers on the network resolve this one by.
    pub alias: ServiceName,
    /// Label keys must be non-empty and hold no `=`. The caller sets the app,
    /// service and copy labels.
    pub labels: BTreeMap<String, String>,
    /// Delivered through an env file, never on the command line.
    pub env: BTreeMap<String, String>,
    pub ports: Vec<PortBinding>,
    pub mounts: Vec<Mount>,
    /// First element becomes `--entrypoint`, the rest lead the arguments.
    pub entrypoint: Option<Vec<String>>,
    pub command: Option<Vec<String>>,
    pub working_dir: Option<String>,
    pub user: Option<String>,
    pub stop_signal: StopSignal,
    pub stop_grace: Duration,
    pub healthcheck: Option<HealthCheck>,
}

fn pair(args: &mut Vec<OsString>, flag: &str, value: impl AsRef<OsStr>) {
    args.push(flag.into());
    args.push(value.as_ref().to_os_string());
}

fn port_flag(port: &PortBinding) -> String {
    let host_ip = match port.host_ip {
        HostIp::Loopback => "127.0.0.1:",
        HostIp::All => "",
    };
    let host_port = port.host_port.map(|p| p.to_string()).unwrap_or_default();
    let protocol = match port.protocol {
        Protocol::Tcp => "tcp",
        Protocol::Udp => "udp",
    };
    format!("{host_ip}{host_port}:{}/{protocol}", port.container_port)
}

fn mount_flag(mount: &Mount) -> OsString {
    let mut flag = match &mount.source {
        MountSource::Volume(name) => OsString::from(format!("{name}:")),
        MountSource::Bind(path) => {
            let mut source = path.as_os_str().to_os_string();
            source.push(":");
            source
        }
        MountSource::Anonymous => OsString::new(),
    };
    flag.push(mount.target.as_os_str());
    if mount.read_only && mount.source != MountSource::Anonymous {
        flag.push(":ro");
    }
    flag
}

fn seconds(d: Duration) -> String {
    format!("{}s", d.as_secs())
}

fn health_flags(args: &mut Vec<OsString>, health: &HealthCheck) {
    match &health.test {
        HealthTest::Shell(line) => pair(args, "--health-cmd", line),
        HealthTest::Exec(argv) => {
            let line = shlex::try_join(argv.iter().map(String::as_str)).unwrap_or_default();
            pair(args, "--health-cmd", line);
        }
        HealthTest::Disabled => args.push("--no-healthcheck".into()),
    }
    if let Some(d) = health.interval {
        pair(args, "--health-interval", seconds(d));
    }
    if let Some(d) = health.timeout {
        pair(args, "--health-timeout", seconds(d));
    }
    if let Some(n) = health.retries {
        pair(args, "--health-retries", n.to_string());
    }
    if let Some(d) = health.start_period {
        pair(args, "--health-start-period", seconds(d));
    }
}

/// Build the argument list for `engine run`, after the program name.
pub(crate) fn run_args(spec: &RunSpec, env_file: &Path) -> Vec<OsString> {
    let mut args: Vec<OsString> = vec!["run".into(), "-d".into()];
    pair(&mut args, "--name", spec.name.as_str());
    pair(&mut args, "--restart", "no");
    pair(&mut args, "--network", spec.network.as_str());
    pair(&mut args, "--network-alias", spec.alias.as_str());
    for (key, value) in &spec.labels {
        pair(&mut args, "--label", format!("{key}={value}"));
    }
    pair(&mut args, "--env-file", env_file);
    for port in &spec.ports {
        pair(&mut args, "-p", port_flag(port));
    }
    for mount in &spec.mounts {
        pair(&mut args, "-v", mount_flag(mount));
    }
    if let Some(dir) = &spec.working_dir {
        pair(&mut args, "--workdir", dir);
    }
    if let Some(user) = &spec.user {
        pair(&mut args, "--user", user);
    }
    pair(&mut args, "--stop-signal", spec.stop_signal.as_str());
    pair(
        &mut args,
        "--stop-timeout",
        spec.stop_grace.as_secs().to_string(),
    );
    if let Some(health) = &spec.healthcheck {
        health_flags(&mut args, health);
    }
    let entrypoint = spec.entrypoint.as_deref().unwrap_or_default();
    if let Some(program) = spec.entrypoint.as_ref() {
        pair(
            &mut args,
            "--entrypoint",
            program.first().map_or("", String::as_str),
        );
    }
    args.push((&spec.image).into());
    args.extend(entrypoint.iter().skip(1).map(OsString::from));
    args.extend(spec.command.iter().flatten().map(OsString::from));
    args
}

#[cfg(test)]
#[path = "run_args_tests.rs"]
mod tests;
