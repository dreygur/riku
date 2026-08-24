//! Building a [`Sandbox`] from declared capabilities and applying it to a child.
//!
//! The spec is computed in the parent (pure, testable); the OS restrictions run
//! in the child via `Command::pre_exec`, after `fork()` and before `exec()`, so
//! they bind the plugin and survive into the new image.

use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;

use crate::manifest::Capabilities;

use super::enforce;
use super::paths::{Resolved, SandboxPaths};

/// A resolved restriction plan for one plugin invocation.
#[derive(Clone, Debug)]
pub struct Sandbox {
    /// `false` for `privileged` plugins: the operator has opted them out, so
    /// no restriction is applied at all.
    enabled: bool,
    /// Directories the plugin may write to (read/exec stays globally allowed).
    write_paths: Vec<PathBuf>,
    /// Whether TCP networking is permitted.
    allow_network: bool,
}

impl Sandbox {
    /// Translate declared capabilities + invocation paths into a plan.
    ///
    /// Unknown `writes` targets are logged and dropped (never silently granted).
    /// The system temp dir is always writable, since plugins routinely need
    /// scratch space and confining that is more disruptive than valuable.
    pub fn from_capabilities(caps: &Capabilities, paths: &SandboxPaths) -> Self {
        if caps.privileged {
            return Self {
                enabled: false,
                write_paths: Vec::new(),
                allow_network: true,
            };
        }

        let mut write_paths = Vec::new();
        for target in &caps.writes {
            match paths.resolve(target) {
                Resolved::Path(p) => write_paths.push(p),
                Resolved::Unavailable => {}
                Resolved::Unknown => {
                    tracing::warn!(target = %target, "ignoring unknown plugin write target")
                }
            }
        }
        let tmp = std::env::temp_dir();
        if tmp.is_dir() {
            write_paths.push(tmp);
        }

        Self {
            enabled: true,
            write_paths,
            allow_network: caps.network,
        }
    }

    /// Attach the restrictions to `cmd` as a `pre_exec` hook. A no-op for a
    /// privileged (opted-out) plugin.
    pub fn harden(&self, cmd: &mut Command) {
        if !self.enabled {
            return;
        }
        let write_paths = self.write_paths.clone();
        let allow_network = self.allow_network;

        // SAFETY: the closure runs in the forked child before exec. It only
        // makes syscalls and a best-effort stderr write: no locks, no parent
        // heap mutation: so it is safe across fork.
        unsafe {
            cmd.pre_exec(move || enforce::install(&write_paths, allow_network));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn caps(network: bool, writes: &[&str], privileged: bool) -> Capabilities {
        Capabilities {
            network,
            writes: writes.iter().map(|s| s.to_string()).collect(),
            privileged,
        }
    }

    fn ctx() -> SandboxPaths {
        SandboxPaths {
            app_path: Some(PathBuf::from("/srv/apps/web")),
            data_path: Some(PathBuf::from("/srv/data/web")),
            env_path: None,
        }
    }

    #[test]
    fn privileged_disables_the_sandbox() {
        let s = Sandbox::from_capabilities(&caps(false, &["app_dir"], true), &ctx());
        assert!(!s.enabled);
        assert!(s.allow_network);
    }

    #[test]
    fn declared_writes_resolve_to_paths_plus_tmp() {
        let s = Sandbox::from_capabilities(&caps(false, &["app_dir", "data_dir"], false), &ctx());
        assert!(s.enabled);
        assert!(s.write_paths.iter().any(|p| p.ends_with("apps/web")));
        assert!(s.write_paths.iter().any(|p| p.ends_with("data/web")));
        // temp dir is always granted
        assert!(s.write_paths.iter().any(|p| *p == std::env::temp_dir()));
    }

    #[test]
    fn unknown_and_unavailable_targets_are_dropped() {
        // env_dir is unavailable (None), "bogus" is unknown, neither grants a path.
        let s = Sandbox::from_capabilities(&caps(true, &["env_dir", "bogus"], false), &ctx());
        assert!(s.allow_network);
        // only the always-on temp dir remains
        assert_eq!(s.write_paths, vec![std::env::temp_dir()]);
    }
}
