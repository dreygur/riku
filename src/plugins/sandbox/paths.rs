//! The `writes` capability vocabulary and its resolution to real directories.
//!
//! A manifest declares write targets symbolically (`app_dir`, `data_dir`,
//! `env_dir`) rather than as absolute paths, so the manifest stays portable and
//! the kernel — not the plugin — decides what each name maps to. This module is
//! the single source of truth for that mapping.

use std::path::PathBuf;

/// The concrete directories a plugin invocation *could* be granted write access
/// to, resolved from the invocation's context. A field is `None` when the seam
/// does not provide that location (e.g. a router has no `data_dir`).
#[derive(Clone, Debug, Default)]
pub struct SandboxPaths {
    /// The app's checked-out source (`RIKU_APP_PATH`).
    pub app_path: Option<PathBuf>,
    /// An addon instance's data directory (`RIKU_ADDON_DATA_PATH`).
    pub data_path: Option<PathBuf>,
    /// The app's ENV directory (`RIKU_ENV_PATH`).
    pub env_path: Option<PathBuf>,
}

/// Resolution of a declared write target: a known name mapped to a present
/// path, a known name whose path this invocation does not provide, or an
/// unrecognized name.
pub(super) enum Resolved {
    /// The target is known and its directory is available.
    Path(PathBuf),
    /// The target name is valid but not available in this context (e.g.
    /// `data_dir` requested by a non-addon). Granted nothing; not an error.
    Unavailable,
    /// The target name is not part of the vocabulary — a manifest typo or a
    /// privilege the kernel will not grant.
    Unknown,
}

impl SandboxPaths {
    /// Map one declared `writes` entry to a directory, if any.
    pub(super) fn resolve(&self, target: &str) -> Resolved {
        let slot = match target {
            "app_dir" => &self.app_path,
            "data_dir" => &self.data_path,
            "env_dir" => &self.env_path,
            _ => return Resolved::Unknown,
        };
        match slot {
            Some(p) => Resolved::Path(p.clone()),
            None => Resolved::Unavailable,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths() -> SandboxPaths {
        SandboxPaths {
            app_path: Some(PathBuf::from("/srv/apps/web")),
            data_path: None,
            env_path: Some(PathBuf::from("/srv/envs/web")),
        }
    }

    #[test]
    fn resolves_known_present_targets() {
        assert!(matches!(paths().resolve("app_dir"), Resolved::Path(p) if p.ends_with("web")));
        assert!(matches!(paths().resolve("env_dir"), Resolved::Path(_)));
    }

    #[test]
    fn known_but_absent_is_unavailable_not_unknown() {
        assert!(matches!(paths().resolve("data_dir"), Resolved::Unavailable));
    }

    #[test]
    fn unrecognized_target_is_unknown() {
        assert!(matches!(paths().resolve("etc"), Resolved::Unknown));
        assert!(matches!(paths().resolve("/etc"), Resolved::Unknown));
    }
}
