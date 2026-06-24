//! Capability enforcement for plugin child processes (`PLUGIN_PROTOCOL.md` §3, §10).
//!
//! Turns a plugin's *declared* `[capabilities]` into OS-level restrictions on
//! the child process, so a plugin can do only what its manifest claimed.
//! Enforcement is **unprivileged** (no root, no setup) and **best-effort** —
//! the manifest's "enforced where possible": on a kernel without Landlock the
//! filesystem/network limits degrade to no-ops (warned on the deploy log),
//! while `no_new_privs` applies everywhere.
//!
//! Security model:
//! - `privileged = true` → the operator opts the plugin out of the sandbox; it
//!   runs with the deploy user's full ambient authority. This is the one knob
//!   that *widens* access, so it is surfaced at install time (§10).
//! - `privileged = false` (default) → `PR_SET_NO_NEW_PRIVS` blocks setuid
//!   privilege escalation (and is required for unprivileged Landlock).
//! - `writes = [app_dir|data_dir|env_dir]` → Landlock filesystem ruleset:
//!   read/execute is allowed globally; write/create/delete is confined to the
//!   declared targets (plus the system temp dir). Undeclared writes ⇒ denied.
//! - `network = false` → Landlock network ruleset denies all TCP bind/connect
//!   (kernel ≥6.7). UDP and non-TCP protocols are out of scope for now and are
//!   *not* blocked — declaring `network = false` is not yet a UDP guarantee.
//!
//! Legacy runtime plugins without a manifest (§8) carry no declared
//! capabilities and are spawned through their own path, so they are unaffected.

mod apply;
mod paths;

pub use apply::Sandbox;
pub use paths::SandboxPaths;

use std::process::Command;

use crate::plugins::manifest::Capabilities;

/// Convenience: build the [`Sandbox`] for `caps`/`paths` and attach it to `cmd`.
/// Call this on every manifest-based plugin spawn, right before launching.
pub fn harden(cmd: &mut Command, caps: &Capabilities, paths: &SandboxPaths) {
    Sandbox::from_capabilities(caps, paths).harden(cmd);
}
