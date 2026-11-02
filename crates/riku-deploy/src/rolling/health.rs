//! Waiting for a freshly started copy to become healthy.
//!
//! # Security model
//!
//! Probes only ever connect to `127.0.0.1` on a port riku chose. The health
//! path comes from a compose label or an app ENV value, so it is forced to
//! start with `/` (it cannot change the host) and redirects are not followed
//! (a copy cannot point the probe at another service). A 3xx answer therefore
//! counts as unhealthy. System proxy settings are ignored for the same reason.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{anyhow, Result};
use riku_container::compose::ServiceDef;
use riku_container::engine::{ContainerId, ContainerState, Engine};
use riku_container::types::labels;
use riku_container::types::HealthTest;

use super::probe::{poll_probe, poll_until, probe_client, probe_port, probe_socket, probe_url};

#[cfg(not(test))]
const HEALTHCHECK_WAIT: Duration = Duration::from_secs(120);
#[cfg(test)]
const HEALTHCHECK_WAIT: Duration = Duration::from_secs(2);
#[cfg(not(test))]
const PROBE_WAIT: Duration = Duration::from_secs(60);
#[cfg(test)]
const PROBE_WAIT: Duration = Duration::from_secs(1);

/// What to watch to decide a new copy is ready for traffic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealthTarget {
    /// The engine's own health check for this container.
    Engine(ContainerId),
    /// An HTTP GET on `127.0.0.1:port` that must answer 2xx.
    Url { port: u16, path: String },
    /// A TCP connect to `127.0.0.1:port` that must succeed.
    Port(u16),
    /// A connect to the unix socket a uWSGI copy listens on that must succeed.
    Socket(PathBuf),
}

/// Pick how to check a compose service copy, or `None` when started is
/// enough (a worker with no health check and no port).
pub fn health_target(
    service: &ServiceDef,
    id: &ContainerId,
    host_port: Option<u16>,
) -> Option<HealthTarget> {
    let has_engine_check = match &service.healthcheck {
        Some(check) => check.test != HealthTest::Disabled,
        None => false,
    };
    if has_engine_check {
        return Some(HealthTarget::Engine(id.clone()));
    }
    let port = match host_port {
        Some(port) => port,
        None => return None,
    };
    match service.labels.get(labels::HEALTH) {
        Some(path) => Some(HealthTarget::Url {
            port,
            path: path.clone(),
        }),
        None => Some(HealthTarget::Port(port)),
    }
}

/// Pick how to check a Procfile web process: `RIKU_HEALTH_CHECK_URL` (a path)
/// when the app sets it, else an open port.
pub fn procfile_target(env: &HashMap<String, String>, port: u16) -> HealthTarget {
    match env.get("RIKU_HEALTH_CHECK_URL") {
        Some(path) => HealthTarget::Url {
            port,
            path: path.clone(),
        },
        None => HealthTarget::Port(port),
    }
}

/// One engine poll. `Ok(true)` is healthy, `Ok(false)` keep waiting, `Err`
/// means the copy will not get healthy.
fn engine_state_ready(engine: &Engine, id: &ContainerId) -> Result<bool> {
    let infos = match engine.inspect(std::slice::from_ref(id)) {
        Ok(infos) => infos,
        Err(e) => return Err(e),
    };
    let info = match infos.first() {
        Some(info) => info,
        None => return Err(anyhow!("container {} is gone", id.as_str())),
    };
    match &info.state {
        ContainerState::Running => Ok(true),
        ContainerState::Unhealthy => Err(anyhow!("container {} reported unhealthy", info.name)),
        ContainerState::Exited { code } => {
            Err(anyhow!("container {} exited with code {code}", info.name))
        }
        ContainerState::Starting
        | ContainerState::Restarting
        | ContainerState::Created
        | ContainerState::Other(_) => Ok(false),
    }
}

/// Block until `target` is healthy or its time limit passes.
///
/// `engine` is required for [`HealthTarget::Engine`] and ignored otherwise.
/// An engine check fails at once when the container is unhealthy or has
/// exited; a probe keeps retrying until the limit.
///
/// Blocks the calling thread for up to the limit and uses the blocking HTTP
/// client, so it must not run on a tokio runtime thread.
pub fn wait_healthy(target: &HealthTarget, engine: Option<&Engine>) -> Result<()> {
    match target {
        HealthTarget::Engine(id) => {
            let engine = match engine {
                Some(engine) => engine,
                None => return Err(anyhow!("an engine health check needs an engine")),
            };
            poll_until(HEALTHCHECK_WAIT, || engine_state_ready(engine, id))
        }
        HealthTarget::Url { port, path } => {
            let client = match probe_client() {
                Ok(client) => client,
                Err(e) => return Err(e),
            };
            poll_probe(PROBE_WAIT, || probe_url(&client, *port, path))
        }
        HealthTarget::Port(port) => poll_probe(PROBE_WAIT, || probe_port(*port)),
        HealthTarget::Socket(path) => poll_probe(PROBE_WAIT, || probe_socket(path)),
    }
}

#[cfg(test)]
#[path = "health_tests.rs"]
mod tests;
