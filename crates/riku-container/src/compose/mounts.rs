//! Mounts as a compose service declares them.
//!
//! # Security model
//!
//! Engine volume names are shared by every app on the server. A compose
//! service names a volume by its top level key, which is only meaningful
//! inside its project, so a [`ServiceMount`] holds a [`VolumeKey`] and is a
//! different type from the engine's [`Mount`]. The only way from one to the
//! other is [`ComposeProject::engine_mounts`], which qualifies each key as
//! `<app>.<key>` with the riku app name. Keys never contain `.` (see
//! `volume_decl`), so the name splits back at its last `.` and two apps cannot
//! share a volume.
//!
//! Handing a service's mounts straight to the engine does not compile:
//!
//! ```compile_fail
//! use riku_container::compose::ServiceDef;
//! use riku_container::types::Mount;
//!
//! fn engine_mounts(service: ServiceDef) -> Vec<Mount> {
//!     service.mounts
//! }
//! ```

use super::{ComposeProject, ServiceDef};
use crate::types::{check_app_name, Mount, MountSource};
use anyhow::{anyhow, Result};
use std::fmt;
use std::path::PathBuf;

/// A key under the compose file's top level `volumes`, as a service names it.
/// Not an engine volume name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VolumeKey(String);

impl VolumeKey {
    /// Wrap a key the parser has checked against the declared volumes.
    pub(super) fn declared(key: &str) -> VolumeKey {
        VolumeKey(key.to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for VolumeKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// What backs a service mount.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceMountSource {
    /// A volume declared at the top level of the compose file.
    Volume(VolumeKey),
    /// A host path inside the app directory, absolute.
    Bind(PathBuf),
    /// A volume the engine creates for this container only.
    Anonymous,
}

/// One mount of a compose service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceMount {
    pub source: ServiceMountSource,
    /// Absolute path inside the container.
    pub target: PathBuf,
    pub read_only: bool,
}

impl ServiceMount {
    /// True when the container can write through this mount. A service with
    /// such a mount holds state and cannot be scaled.
    pub fn is_writable(&self) -> bool {
        !self.read_only
    }
}

impl ComposeProject {
    /// The engine mounts for `service` of the riku app `app`, with every
    /// volume key qualified as `<app>.<key>`.
    ///
    /// Fails for an app name the engine cannot take and for a key this project
    /// does not declare.
    pub fn engine_mounts(&self, app: &str, service: &ServiceDef) -> Result<Vec<Mount>> {
        if let Err(e) = check_app_name(app) {
            return Err(e);
        }
        service
            .mounts
            .iter()
            .map(|mount| self.engine_mount(app, service, mount))
            .collect()
    }

    fn engine_mount(&self, app: &str, service: &ServiceDef, mount: &ServiceMount) -> Result<Mount> {
        let source = match &mount.source {
            ServiceMountSource::Volume(key) if self.volumes().contains_key(key.as_str()) => {
                MountSource::Volume(format!("{app}.{key}"))
            }
            ServiceMountSource::Volume(key) => {
                return Err(anyhow!(
                    "service '{}' mounts volume '{key}', which app '{app}' does not declare",
                    service.name
                ))
            }
            ServiceMountSource::Bind(path) => MountSource::Bind(path.clone()),
            ServiceMountSource::Anonymous => MountSource::Anonymous,
        };
        Ok(Mount {
            source,
            target: mount.target.clone(),
            read_only: mount.read_only,
        })
    }
}

#[cfg(test)]
#[path = "mounts_tests.rs"]
mod tests;
