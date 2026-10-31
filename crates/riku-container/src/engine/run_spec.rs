//! What one container is started with.

use std::collections::BTreeMap;
use std::fmt;
use std::time::Duration;

use crate::types::{
    ContainerName, HealthCheck, Mount, NetworkName, PortBinding, ServiceName, StopSignal,
};

/// Whether the engine may fetch the image from a registry when it is missing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PullPolicy {
    /// The engine's default: pull when the image is not present locally.
    #[default]
    Missing,
    /// Never pull (`--pull=never`). For images riku built itself, so a missing
    /// image fails the start instead of running a same named registry image.
    Never,
}

/// Everything needed to start one container.
///
/// An `Exec` health check is joined into a shell line because the engine CLI
/// has no exec form, so an image without `/bin/sh` reports unhealthy.
///
/// The `Debug` output lists env names but never values, which may be secrets.
#[derive(Clone)]
pub struct RunSpec {
    pub name: ContainerName,
    /// Image reference, checked by `Engine::run` before use.
    pub image: String,
    pub pull: PullPolicy,
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
    /// Rounded up to whole seconds when passed to the engine.
    pub stop_grace: Duration,
    pub healthcheck: Option<HealthCheck>,
}

impl fmt::Debug for RunSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RunSpec")
            .field("name", &self.name)
            .field("image", &self.image)
            .field("pull", &self.pull)
            .field("network", &self.network)
            .field("alias", &self.alias)
            .field("labels", &self.labels)
            .field("env_names", &self.env.keys().collect::<Vec<_>>())
            .field("ports", &self.ports)
            .field("mounts", &self.mounts)
            .field("entrypoint", &self.entrypoint)
            .field("command", &self.command)
            .field("working_dir", &self.working_dir)
            .field("user", &self.user)
            .field("stop_signal", &self.stop_signal)
            .field("stop_grace", &self.stop_grace)
            .field("healthcheck", &self.healthcheck)
            .finish()
    }
}
