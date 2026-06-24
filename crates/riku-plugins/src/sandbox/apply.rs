//! Building a [`Sandbox`] from declared capabilities and applying it to a child.
//!
//! The spec is computed in the parent (pure, testable); the OS restrictions run
//! in the child via `Command::pre_exec`, after `fork()` and before `exec()`, so
//! they bind the plugin and survive into the new image.

use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;

use crate::manifest::Capabilities;

use super::paths::{Resolved, SandboxPaths};

/// A resolved restriction plan for one plugin invocation.
#[derive(Clone, Debug)]
pub struct Sandbox {
    /// `false` for `privileged` plugins — the operator has opted them out, so
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
        // makes syscalls (prctl, Landlock) and a best-effort stderr write — no
        // locks, no parent heap mutation — so it is safe across fork.
        unsafe {
            cmd.pre_exec(move || {
                set_no_new_privs()?;
                if apply_landlock(&write_paths, allow_network).is_err() {
                    warn_unenforced();
                }
                Ok(())
            });
        }
    }
}

/// `PR_SET_NO_NEW_PRIVS` — stop the plugin gaining privileges via setuid/setgid
/// binaries, and satisfy Landlock's unprivileged precondition. A hard failure
/// here is unexpected and fails the spawn closed.
fn set_no_new_privs() -> std::io::Result<()> {
    // SAFETY: prctl with these args has no memory effects.
    let rc = unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) };
    if rc != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

/// Build and enforce the Landlock ruleset: read/execute everywhere, write only
/// under `write_paths`, and (when `allow_network` is false) deny all TCP
/// bind/connect. Best-effort: on a kernel without (full) Landlock the crate
/// downgrades the ruleset rather than erroring.
fn apply_landlock(
    write_paths: &[PathBuf],
    allow_network: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use landlock::{
        Access, AccessFs, AccessNet, CompatLevel, Compatible, PathBeneath, PathFd, Ruleset,
        RulesetAttr, RulesetCreatedAttr, ABI,
    };

    // Request the newest ABI; BestEffort negotiates down to what the kernel has.
    let abi = ABI::V5;
    let mut ruleset = Ruleset::default()
        .set_compatibility(CompatLevel::BestEffort)
        .handle_access(AccessFs::from_all(abi))?;
    if !allow_network {
        ruleset = ruleset.handle_access(AccessNet::BindTcp | AccessNet::ConnectTcp)?;
    }

    let mut created = ruleset.create()?;
    // Read + execute across the whole filesystem so the plugin can run and read
    // libraries/config; write rights are governed separately below.
    created = created.add_rule(PathBeneath::new(
        PathFd::new("/")?,
        AccessFs::from_read(abi),
    ))?;
    // Device nodes must stay writable: shells and tools constantly open
    // /dev/null, /dev/stdout, /dev/tty, ptys, etc. Creating files here needs
    // privilege anyway, so granting full access is harmless.
    if let Ok(fd) = PathFd::new("/dev") {
        created = created.add_rule(PathBeneath::new(fd, AccessFs::from_all(abi)))?;
    }
    for path in write_paths {
        // A path that cannot be opened (removed/racing) is skipped rather than
        // aborting the whole sandbox.
        if let Ok(fd) = PathFd::new(path) {
            created = created.add_rule(PathBeneath::new(fd, AccessFs::from_all(abi)))?;
        }
    }

    // No NetPort allow-rules were added, so every handled TCP access is denied.
    created.restrict_self()?;
    Ok(())
}

/// Warn (on the child's stderr, which is streamed to the deploy log) that
/// confinement could not be installed, then let the plugin run. Uses a raw
/// write to stay simple in the post-fork context.
fn warn_unenforced() {
    const MSG: &[u8] = b"riku: warning: plugin sandbox could not be enforced on this host\n";
    // SAFETY: a single write to the inherited stderr fd; ignore the result.
    unsafe {
        libc::write(2, MSG.as_ptr() as *const libc::c_void, MSG.len());
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
        // env_dir is unavailable (None), "bogus" is unknown — neither grants a path.
        let s = Sandbox::from_capabilities(&caps(true, &["env_dir", "bogus"], false), &ctx());
        assert!(s.allow_network);
        // only the always-on temp dir remains
        assert_eq!(s.write_paths, vec![std::env::temp_dir()]);
    }
}
