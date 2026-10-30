//! Mounts as a compose service declares them.
//!
//! # Security model
//!
//! Engine volume names are shared by every app on the server. A compose
//! service names a volume by its top level key, which is only meaningful
//! inside its project, so a [`ServiceMount`] holds a [`VolumeKey`] and is a
//! different type from the engine's [`Mount`]. The only way from one to the
//! other is [`ComposeProject::engine_mounts`], which qualifies each key as
//! `<project>_<key>` through [`existing_volume_name`] and so refuses a project
//! whose declared name is not confirmed.
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

use super::{existing_volume_name, ComposeProject, ServiceDef};
use crate::types::{Mount, MountSource};
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
    /// The engine mounts for `service`, with every volume key qualified as
    /// `<project>_<key>`.
    ///
    /// Fails for a volume of a project whose name is still `Declared`, and for
    /// a key this project does not declare.
    pub fn engine_mounts(&self, service: &ServiceDef) -> Result<Vec<Mount>> {
        service
            .mounts
            .iter()
            .map(|mount| self.engine_mount(service, mount))
            .collect()
    }

    fn engine_mount(&self, service: &ServiceDef, mount: &ServiceMount) -> Result<Mount> {
        let source = match &mount.source {
            ServiceMountSource::Volume(key) => match existing_volume_name(self, key.as_str()) {
                Ok(Some(name)) => MountSource::Volume(name),
                Ok(None) => {
                    return Err(anyhow!(
                        "service '{}' mounts volume '{key}', which project '{}' does not declare",
                        service.name,
                        self.project_name()
                    ))
                }
                Err(e) => return Err(e),
            },
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
