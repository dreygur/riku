//! Post-fork enforcement of a sandbox plan.
//!
//! Everything here runs in the forked child between `fork()` and `exec()`, so
//! it may only make syscalls: no locks, no parent heap mutation.
//!
//! Only Linux has the unprivileged primitives this needs (`PR_SET_NO_NEW_PRIVS`
//! and Landlock). Riku deploys on Linux; other targets exist for development
//! builds only, and there the child runs unconfined with a warning on its
//! stderr rather than silently.

use std::path::PathBuf;

/// Install the restrictions on the current (child) process.
///
/// Failing to set `no_new_privs` is fatal: it is the precondition for
/// unprivileged Landlock, so the spawn fails closed. A Landlock failure is
/// best-effort by design and only warns.
#[cfg(target_os = "linux")]
pub(super) fn install(write_paths: &[PathBuf], allow_network: bool) -> std::io::Result<()> {
    match set_no_new_privs() {
        Ok(()) => {}
        Err(e) => return Err(e),
    }
    if apply_landlock(write_paths, allow_network).is_err() {
        warn_unenforced();
    }
    Ok(())
}

/// No unprivileged confinement primitive exists off Linux, so the plugin runs
/// with the deploy user's full authority. Development builds only.
#[cfg(not(target_os = "linux"))]
pub(super) fn install(_write_paths: &[PathBuf], _allow_network: bool) -> std::io::Result<()> {
    warn_unenforced();
    Ok(())
}

/// `PR_SET_NO_NEW_PRIVS` — stop the plugin gaining privileges via setuid/setgid
/// binaries, and satisfy Landlock's unprivileged precondition.
#[cfg(target_os = "linux")]
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
#[cfg(target_os = "linux")]
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
