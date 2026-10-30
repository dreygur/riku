//! The `run -d` argument list for one container.
//!
//! Security model: nothing here is passed through a shell. The environment
//! never appears in the list, only the path of the env file that carries it,
//! so secrets stay out of the process list. Callers run
//! [`super::spec_check::check_spec`] first; [`super::Engine::run`] does.

use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::time::Duration;

use anyhow::{anyhow, Result};

use super::run_spec::RunSpec;
use super::spec_check::exec_command_line;
use crate::types::{HealthCheck, HealthTest, HostIp, Mount, MountSource, PortBinding, Protocol};

/// `d` in whole seconds, rounded up. The engine CLI takes whole seconds, and
/// rounding down would turn `500ms` into `0`, which means the engine default
/// for a health duration and an immediate kill for a stop grace.
pub(crate) fn whole_seconds_up(d: Duration) -> u64 {
    if d.subsec_nanos() == 0 {
        d.as_secs()
    } else {
        d.as_secs() + 1
    }
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

/// `-v` cannot express a read only anonymous volume (`-v /x:ro` names a
/// volume called `/x`), so that one case uses `--mount`.
fn push_mount(args: &mut Vec<OsString>, mount: &Mount) {
    if mount.source == MountSource::Anonymous && mount.read_only {
        let mut spec = OsString::from("type=volume,dst=");
        spec.push(mount.target.as_os_str());
        spec.push(",readonly");
        pair(args, "--mount", spec);
        return;
    }
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
    if mount.read_only {
        flag.push(":ro");
    }
    pair(args, "-v", flag);
}

fn seconds(d: Duration) -> String {
    format!("{}s", whole_seconds_up(d))
}

fn health_flags(args: &mut Vec<OsString>, health: &HealthCheck) -> Result<()> {
    match &health.test {
        HealthTest::Shell(line) => pair(args, "--health-cmd", line),
        HealthTest::Exec(argv) => match exec_command_line(argv) {
            Ok(line) => pair(args, "--health-cmd", line),
            Err(e) => return Err(anyhow!("invalid health check: {e}")),
        },
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
    Ok(())
}

/// Build the argument list for `engine run`, after the program name.
///
/// Fails when an exec form health check is empty or cannot be quoted.
pub(crate) fn run_args(spec: &RunSpec, env_file: &Path) -> Result<Vec<OsString>> {
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
        push_mount(&mut args, mount);
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
        whole_seconds_up(spec.stop_grace).to_string(),
    );
    if let Some(health) = &spec.healthcheck {
        match health_flags(&mut args, health) {
            Ok(()) => {}
            Err(e) => return Err(e),
        }
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
    Ok(args)
}

#[cfg(test)]
#[path = "run_args_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "run_args_health_tests.rs"]
mod health_tests;
