//! Container, service and network names.
//!
//! Every constructor enforces the character set an engine accepts for a name
//! (`[a-zA-Z0-9][a-zA-Z0-9_.-]*`), so a value of these types cannot start with
//! `-` and be read as an option, or carry a path separator or whitespace.

use std::fmt;

/// Label keys riku sets on containers and reads from compose services.
pub mod labels {
    /// Owning app, set by riku on every container.
    pub const APP: &str = "riku.app";
    /// Service name within the app, set by riku on every container.
    pub const SERVICE: &str = "riku.service";
    /// Copy number of the service, set by riku on every container.
    pub const COPY: &str = "riku.copy";
    /// Compose opt-in (`"true"`) to publish a service's ports on all interfaces.
    pub const PUBLIC: &str = "riku.public";
    /// Compose label naming the published port that carries routed HTTP.
    pub const HTTP: &str = "riku.http";
    /// Compose label that overrides how riku checks a service's health.
    pub const HEALTH: &str = "riku.health";
    /// Host port riku published for the service, set by riku.
    pub const PORT: &str = "riku.port";
    /// Restart policy riku applies itself, set by riku. The engine's own
    /// restart policy stays off.
    pub const RESTART: &str = "riku.restart";
    /// Label compose tools put on every container of a project.
    pub const COMPOSE_PROJECT: &str = "com.docker.compose.project";
}

const SERVICE_NAME_MAX: usize = 63;

/// True for `[a-zA-Z0-9][a-zA-Z0-9_.-]*`, the names an engine takes without
/// reading them as an option, a path or a second argument.
pub(crate) fn is_engine_safe(name: &str) -> bool {
    let mut chars = name.chars();
    let first_ok = chars.next().is_some_and(|c| c.is_ascii_alphanumeric());
    first_ok && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
}

/// Accepts a project, volume or network name the engine takes as one argument
/// or inside a label filter.
pub(crate) fn check_engine_name(what: &str, name: &str) -> anyhow::Result<()> {
    if is_engine_safe(name) {
        return Ok(());
    }
    Err(anyhow::anyhow!(
        "{what} {name:?} cannot be used with the container engine: it must start with an ASCII letter or digit and use only ASCII letters, digits, '_', '.' and '-'"
    ))
}

/// Accepts an app name that `riku_util::validate_app_name` returns unchanged
/// and that the engine accepts inside a container name or a label filter.
pub fn check_app_name(app: &str) -> anyhow::Result<()> {
    let validated = match riku_util::validate_app_name(app) {
        Ok(v) => v,
        Err(e) => return Err(anyhow::anyhow!("invalid app name {app:?}: {e}")),
    };
    if validated != app {
        return Err(anyhow::anyhow!(
            "invalid app name {app:?}: not a valid app name as written"
        ));
    }
    if !is_engine_safe(app) {
        return Err(anyhow::anyhow!(
            "app name {app:?} cannot be used with the container engine: it must start with an ASCII letter or digit and use only ASCII letters, digits, '_', '.' and '-'"
        ));
    }
    Ok(())
}

/// Name of a service within an app, as written in the compose file. It never
/// contains `.`, which keeps [`ContainerName`] unambiguous.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ServiceName(String);

impl ServiceName {
    /// The validated name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<&str> for ServiceName {
    type Error = anyhow::Error;

    fn try_from(raw: &str) -> anyhow::Result<Self> {
        if raw.contains('.') {
            return Err(anyhow::anyhow!(
                "invalid service name {raw:?}: service names may not contain '.'"
            ));
        }
        if raw.len() > SERVICE_NAME_MAX || !is_engine_safe(raw) {
            return Err(anyhow::anyhow!(
                "invalid service name {raw:?}: use 1 to {SERVICE_NAME_MAX} letters, digits, '_' or '-', starting with a letter or digit"
            ));
        }
        Ok(ServiceName(raw.to_string()))
    }
}

impl fmt::Display for ServiceName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Position of a container among the copies of one service, counted from 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CopyIndex(u32);

impl CopyIndex {
    /// Returns `None` for 0, because copies are numbered from 1.
    pub fn new(n: u32) -> Option<CopyIndex> {
        if n == 0 {
            None
        } else {
            Some(CopyIndex(n))
        }
    }

    /// The copy number, at least 1.
    pub fn get(self) -> u32 {
        self.0
    }
}

impl fmt::Display for CopyIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Engine name of one container, `<app>.<service>.<copy>`.
///
/// The name reads back unambiguously from the right: the copy is digits only
/// and the service never contains `.`, so the copy follows the last `.`, the
/// service sits between the last two, and everything before them is the app.
/// App names may contain `.` (`riku_util::validate_app_name` allows it), which
/// is why the split has to start from the right. A `-` separator
/// would not work: app `shop` with service `api-web` and app `shop-api` with
/// service `web` would share `shop-api-web-1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ContainerName(String);

impl ContainerName {
    /// Joins the three parts. Fails when `app` is not a name
    /// `riku_util::validate_app_name` returns unchanged, or when it uses a
    /// character the engine does not accept in a container name (that
    /// validator allows a leading `_`, `.` or `-` and non-ASCII letters).
    pub fn new(app: &str, service: &ServiceName, copy: CopyIndex) -> anyhow::Result<ContainerName> {
        match check_app_name(app) {
            Ok(()) => {}
            Err(e) => return Err(e),
        }
        Ok(ContainerName(format!("{app}.{service}.{copy}")))
    }

    /// The one-off container that runs an app's Procfile `release` command:
    /// `<app>-release`. It cannot collide with a copy, whose name ends in a
    /// number, and two apps' release names differ because their app names do.
    /// Fails on the same `app` values as [`ContainerName::new`].
    pub fn for_release(app: &str) -> anyhow::Result<ContainerName> {
        match check_app_name(app) {
            Ok(()) => {}
            Err(e) => return Err(e),
        }
        Ok(ContainerName(format!("{app}-release")))
    }

    /// The name to pass to the engine.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Engine network that holds all containers of one app.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NetworkName(String);

impl NetworkName {
    /// The network is `riku-<app>`. `app` must already be validated.
    pub fn for_app(app: &str) -> NetworkName {
        NetworkName(format!("riku-{app}"))
    }

    /// The name to pass to the engine.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
#[path = "names_tests.rs"]
mod tests;
