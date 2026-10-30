//! The typed result of parsing a service, and the context it is parsed in.
//!
//! The `Debug` output of [`ServiceContext`], [`ServiceDef`] and [`BuildDef`]
//! lists variable, environment and build arg names but never values, which may
//! be secrets.

use crate::types::{HealthCheck, Mount, PortBinding, RestartPolicy, ServiceName, StopSignal};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// What `parse_service` needs to know about the project around a service.
#[derive(Clone, Copy)]
pub struct ServiceContext<'a> {
    /// The app checkout. Nothing a service names may resolve outside it.
    pub app_path: &'a Path,
    /// Directory holding the compose file, inside `app_path`. Relative paths
    /// in the service resolve against it.
    pub compose_dir: &'a Path,
    /// Interpolation variables: the project `.env` overlaid by the app ENV.
    /// A bare `KEY` in `environment` takes its value from here.
    pub vars: &'a BTreeMap<String, String>,
    /// Names under the top level `volumes` key.
    pub declared_volumes: &'a BTreeSet<String>,
}

impl fmt::Debug for ServiceContext<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServiceContext")
            .field("app_path", &self.app_path)
            .field("compose_dir", &self.compose_dir)
            .field("var_names", &self.vars.keys().collect::<Vec<_>>())
            .field("declared_volumes", &self.declared_volumes)
            .finish()
    }
}

/// How a service is built. `context` and `dockerfile` are absolute paths
/// inside the app directory.
#[derive(Clone, PartialEq, Eq)]
pub struct BuildDef {
    pub context: PathBuf,
    pub dockerfile: Option<PathBuf>,
    pub args: BTreeMap<String, String>,
}

impl fmt::Debug for BuildDef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BuildDef")
            .field("context", &self.context)
            .field("dockerfile", &self.dockerfile)
            .field("arg_names", &self.args.keys().collect::<Vec<_>>())
            .finish()
    }
}

/// When a dependency counts as satisfied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DependsCondition {
    Started,
    Healthy,
}

/// A service another service waits for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dependency {
    pub service: ServiceName,
    pub condition: DependsCondition,
}

/// One validated service of a compose project. `None` fields were not set in
/// the compose file and keep the image's own value.
#[derive(Clone, PartialEq, Eq)]
pub struct ServiceDef {
    pub name: ServiceName,
    pub image: Option<String>,
    pub build: Option<BuildDef>,
    pub command: Option<Vec<String>>,
    pub entrypoint: Option<Vec<String>>,
    /// `env_file` values overlaid by `environment`. May hold secrets.
    pub environment: BTreeMap<String, String>,
    pub ports: Vec<PortBinding>,
    pub mounts: Vec<Mount>,
    pub depends_on: Vec<Dependency>,
    pub healthcheck: Option<HealthCheck>,
    pub restart: RestartPolicy,
    pub stop_grace: Duration,
    pub stop_signal: StopSignal,
    pub labels: BTreeMap<String, String>,
    pub working_dir: Option<String>,
    pub user: Option<String>,
}

impl fmt::Debug for ServiceDef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServiceDef")
            .field("name", &self.name)
            .field("image", &self.image)
            .field("build", &self.build)
            .field("command", &self.command)
            .field("entrypoint", &self.entrypoint)
            .field(
                "environment_keys",
                &self.environment.keys().collect::<Vec<_>>(),
            )
            .field("ports", &self.ports)
            .field("mounts", &self.mounts)
            .field("depends_on", &self.depends_on)
            .field("healthcheck", &self.healthcheck)
            .field("restart", &self.restart)
            .field("stop_grace", &self.stop_grace)
            .field("stop_signal", &self.stop_signal)
            .field("labels", &self.labels)
            .field("working_dir", &self.working_dir)
            .field("user", &self.user)
            .finish()
    }
}
