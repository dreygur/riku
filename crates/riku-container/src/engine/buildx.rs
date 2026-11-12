//! Whether a docker engine has the buildx plugin. Without it docker builds
//! with its deprecated legacy builder and prints a notice on every build.

use std::ffi::OsStr;

use super::Engine;

impl Engine {
    /// `None` when the engine is not docker (podman and the others build
    /// without buildx), else whether `docker buildx version` answers.
    pub fn docker_has_buildx(&self) -> Option<bool> {
        if self.program().file_name() != Some(OsStr::new("docker")) {
            return None;
        }
        Some(self.output(&["buildx".into(), "version".into()]).is_ok())
    }
}

#[cfg(test)]
#[path = "buildx_tests.rs"]
mod tests;
