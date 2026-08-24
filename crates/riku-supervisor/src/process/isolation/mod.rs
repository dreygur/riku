//! Linux namespace isolation for spawned worker processes.
//!
//! Applies `CLONE_NEWNS` (mount), `CLONE_NEWNET` (network), and
//! `CLONE_NEWPID` (process) namespaces to a worker, then execs it.
//!
//! # Why this isn't done in `pre_exec`
//!
//! PID namespace isolation needs a fork after `unshare(CLONE_NEWPID)`: per
//! pid_namespaces(7), `unshare(CLONE_NEWPID)` does NOT move the caller into
//! the new namespace, only its *future children*. An earlier version of
//! this module did that extra fork from inside `Command::pre_exec`, i.e.
//! between `fork()` and `execve()` in the worker's own spawn, with the
//! outer (pre_exec) process becoming a signal-forwarding shim that never
//! called `execve` itself, looping until the inner process exited and then
//! calling `_exit` directly.
//!
//! That deadlocked the supervisor. `std::process::Command::spawn()` detects
//! a successful `execve` via a `CLOEXEC` self-pipe: the write end stays open
//! until every process holding it either execs or exits, and `spawn()`
//! blocks reading that pipe until it closes. The pre_exec shim never exec'd
//! and only exited once the *worker* did: so `spawn()` didn't return until
//! the isolated worker's entire lifetime had elapsed, and since
//! `ProcessManager::spawn_process` runs synchronously on the supervisor's
//! single-threaded main loop, that froze health checks, log rotation, cron,
//! and every other app's reload for as long as that one worker ran.
//!
//! The fix: do the unshare/fork/exec dance in a real process, not inside
//! `pre_exec`. `ProcessManager::spawn_process` execs the `riku __ns-shim`
//! subcommand (see `cli::cli::Commands::NsShim`) instead of the worker
//! directly when isolation is enabled. `Command::spawn()` returns as soon as
//! *that* `execve` succeeds: `__ns-shim`'s own `main` is then free to
//! `unshare`, `fork`, and loop as a signal-forwarding shim on its own time,
//! with no effect on the supervisor's `Command::spawn()` call, because that
//! call already returned.
//!
//! # Safety / signal-safety note
//! The mount/pivot_root sequence here does allocate (path joins,
//! `create_dir_all`). Unlike the old pre_exec version, `exec_isolated` runs
//! as a freshly exec'd process's `main`, not between `fork()` and `execve()`
//! of a process some other code is also forking/threading around, so the
//! single-threaded-child signal-safety caveat that applied to `pre_exec`
//! doesn't apply here.

use std::io;
use std::path::{Path, PathBuf};

#[cfg(target_os = "linux")]
mod linux;

/// Namespace isolation settings for a worker process. Used by
/// `spawn_process` to decide whether to exec the worker directly or route
/// it through `riku __ns-shim`: see module docs for why.
#[derive(Debug, Clone, Default)]
pub struct NamespaceConfig {
    /// Master switch. When false, the worker runs with the same namespaces
    /// as the supervisor (today's behavior).
    pub enabled: bool,
    /// Directory the worker's mount namespace is rooted at via
    /// `pivot_root`. Must contain everything the worker needs (its app
    /// directory, libraries, `/proc`, `/dev`, etc.) since the rest of the
    /// host filesystem becomes unreachable. Required when `enabled`.
    pub isolated_root: Option<PathBuf>,
}

/// Set up namespace isolation rooted at `root` and exec `command` (via
/// `sh -c`) inside it. Called from the `riku __ns-shim` subcommand handler,
/// i.e. from a process's own `main`, already past its own `execve`. See
/// module docs for why this can't run inside the worker's `pre_exec`.
///
/// On success this never returns: either the inner process successfully
/// execs the real worker command, or this process becomes the
/// signal-forwarding shim and calls `_exit` once that worker exits. It only
/// returns `Err` if a setup step fails or the final `exec` itself fails.
#[cfg(target_os = "linux")]
pub fn exec_isolated(root: &Path, command: &str) -> io::Result<()> {
    linux::exec_isolated(root, command)
}

/// Namespaces, `pivot_root`, and the loopback ioctls are Linux-only, so an
/// isolation request cannot be honoured here. It fails loudly rather than
/// running the worker unisolated, which would silently drop the guarantee the
/// config asked for. Development builds only; Riku deploys on Linux.
#[cfg(not(target_os = "linux"))]
pub fn exec_isolated(_root: &Path, _command: &str) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "namespace isolation requires Linux",
    ))
}
