//! Volume and bind mounts of a container.

use std::path::PathBuf;

/// What backs a mount.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MountSource {
    /// A named engine volume.
    Volume(String),
    /// A host directory or file.
    Bind(PathBuf),
    /// A volume the engine creates for this container only.
    Anonymous,
}

/// One mount into a container.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mount {
    pub source: MountSource,
    /// Absolute path inside the container.
    pub target: PathBuf,
    pub read_only: bool,
}

impl Mount {
    /// True when the container can write through this mount. A service with
    /// such a mount holds state and cannot be scaled.
    pub fn is_writable(&self) -> bool {
        !self.read_only
    }
}

#[cfg(test)]
#[path = "mounts_tests.rs"]
mod tests;
