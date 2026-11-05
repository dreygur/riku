//! Where the helper's connections come from: a socket systemd already bound,
//! or one the helper creates itself.
//!
//! Security model: a socket the helper creates is owned by root, group the
//! deploy user's primary group, mode 0660. The helper sets umask 0177 at start
//! ([`restrict_umask`]), so the socket is bound as mode 0600 (root only) and
//! only then handed to the group: there is no moment where a wider set of
//! users can connect. A stale socket
//! file is replaced; a socket something still listens on, or any other file
//! at the path, is an error and is never removed.

use std::fs;
use std::os::fd::{FromRawFd, RawFd};
use std::os::unix::fs::{chown, FileTypeExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::time::Duration;

use anyhow::{anyhow, Result};
use nix::fcntl::{fcntl, FcntlArg, FdFlag};
use nix::poll::{poll, PollFd, PollFlags, PollTimeout};
use nix::sys::stat::{umask, Mode};

use crate::protocol::{Reply, Request};
use crate::server::serve_one;

const SOCKET_MODE: u32 = 0o660;
/// Octal 0177: new files are readable and writable by their owner only.
const PRIVATE_UMASK: Mode = Mode::S_IXUSR.union(Mode::S_IRWXG).union(Mode::S_IRWXO);
const FIRST_ACTIVATED_FD: RawFd = 3;

/// The descriptor systemd passed, when `LISTEN_PID` names this process. Any
/// count but one is an error: falling back to binding our own socket would
/// hide a unit that is misconfigured.
fn activated_fd(
    listen_pid: Option<&str>,
    listen_fds: Option<&str>,
    own_pid: u32,
) -> Result<Option<RawFd>> {
    if !listen_pid.is_some_and(|pid| pid.parse() == Ok(own_pid)) {
        return Ok(None);
    }
    match listen_fds {
        Some("1") => Ok(Some(FIRST_ACTIVATED_FD)),
        other => Err(anyhow!(
            "systemd passed {other:?} sockets to the router helper, expected exactly 1"
        )),
    }
}

/// The listener systemd passed to this process, or `None` when it was not
/// socket activated. The variables are cleared so a child process the
/// helper starts does not mistake the descriptor for its own.
pub fn activated_listener() -> Result<Option<UnixListener>> {
    let pid = std::env::var("LISTEN_PID").ok();
    let fds = std::env::var("LISTEN_FDS").ok();
    let fd = activated_fd(pid.as_deref(), fds.as_deref(), std::process::id());
    std::env::remove_var("LISTEN_PID");
    std::env::remove_var("LISTEN_FDS");
    let fd = match fd {
        Ok(Some(fd)) => fd,
        Ok(None) => return Ok(None),
        Err(e) => return Err(anyhow!("socket activation: {e}")),
    };
    // SAFETY: systemd handed this process descriptor 3 as its only socket
    // (LISTEN_PID and LISTEN_FDS matched above) and nothing else owns it.
    let listener = unsafe { UnixListener::from_raw_fd(fd) };
    match fcntl(&listener, FcntlArg::F_SETFD(FdFlag::FD_CLOEXEC)) {
        Ok(_) => Ok(Some(listener)),
        Err(e) => Err(anyhow!(
            "failed to mark the activated socket close-on-exec: {e}"
        )),
    }
}

fn remove_stale_socket(path: &Path) -> Result<()> {
    let kind = match fs::symlink_metadata(path) {
        Ok(meta) => meta.file_type(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(anyhow!("failed to inspect {}: {e}", path.display())),
    };
    if !kind.is_socket() {
        return Err(anyhow!(
            "{} exists and is not a socket, not replacing it",
            path.display()
        ));
    }
    if UnixStream::connect(path).is_ok() {
        return Err(anyhow!(
            "another router helper is listening on {}",
            path.display()
        ));
    }
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) => Err(anyhow!(
            "failed to remove the old socket {}: {e}",
            path.display()
        )),
    }
}

/// Make every file this process creates from now on private to its owner.
/// The helper calls it once at start, before it is multi threaded, so the
/// socket below starts as mode 0600. The umask is process wide, which is why
/// [`bind_socket`] does not set it around the bind.
pub fn restrict_umask() {
    umask(PRIVATE_UMASK);
}

/// Create the socket at `path`, owned by this user with group `gid`, mode
/// 0660. Under [`restrict_umask`] it exists as 0600 until it is handed over.
pub fn bind_socket(path: &Path, gid: u32) -> Result<UnixListener> {
    if let Err(e) = remove_stale_socket(path) {
        return Err(anyhow!("cannot prepare the socket path: {e}"));
    }
    let listener = match UnixListener::bind(path) {
        Ok(listener) => listener,
        Err(e) => return Err(anyhow!("failed to bind {}: {e}", path.display())),
    };
    let shared = chown(path, None, Some(gid))
        .and_then(|()| fs::set_permissions(path, fs::Permissions::from_mode(SOCKET_MODE)));
    match shared {
        Ok(()) => Ok(listener),
        Err(e) => {
            let _ = fs::remove_file(path);
            Err(anyhow!(
                "failed to set the owner and mode of {}: {e}",
                path.display()
            ))
        }
    }
}

fn wait_for_connection(listener: &UnixListener, idle: Duration) -> Result<bool> {
    let timeout = match PollTimeout::try_from(idle) {
        Ok(timeout) => timeout,
        Err(e) => return Err(anyhow!("idle time {idle:?} is out of range: {e}")),
    };
    let mut fds = [PollFd::new(
        std::os::fd::AsFd::as_fd(listener),
        PollFlags::POLLIN,
    )];
    match poll(&mut fds, timeout) {
        Ok(ready) => Ok(ready > 0),
        Err(nix::errno::Errno::EINTR) => Ok(false),
        Err(e) => Err(anyhow!("failed to wait for a connection: {e}")),
    }
}

/// Accept and serve connections one at a time.
///
/// With `idle` set, returns `Ok` once that long passes with no connection
/// waiting (the socket activated case, where systemd starts the helper
/// again). With `None` it serves until killed. A connection that fails is
/// logged and does not stop the loop.
pub fn run(
    listener: &UnixListener,
    idle: Option<Duration>,
    deploy_uid: u32,
    handle: &mut impl FnMut(Request) -> Reply,
) -> Result<()> {
    loop {
        if let Some(idle) = idle {
            match wait_for_connection(listener, idle) {
                Ok(true) => {}
                Ok(false) => return Ok(()),
                Err(e) => return Err(anyhow!("router helper stopped waiting: {e}")),
            }
        }
        let stream = match listener.accept() {
            Ok((stream, _)) => stream,
            Err(e) => return Err(anyhow!("failed to accept a connection: {e}")),
        };
        if let Err(e) = serve_one(stream, deploy_uid, handle) {
            tracing::warn!("router helper connection: {e}");
        }
    }
}

#[cfg(test)]
#[path = "listen_tests.rs"]
mod tests;
