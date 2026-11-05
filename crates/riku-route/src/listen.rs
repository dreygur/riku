//! The socket the helper listens on and the loop that serves it.
//!
//! Security model: the socket is owned by root, group the
//! deploy user's primary group, mode 0660. The helper sets umask 0177 at start
//! ([`restrict_umask`]), so the socket is bound as mode 0600 (root only) and
//! only then handed to the group: there is no moment where a wider set of
//! users can connect. A stale socket
//! file is replaced; a socket something still listens on, or any other file
//! at the path, is an error and is never removed.

use std::fs;
use std::os::unix::fs::{chown, FileTypeExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;

use anyhow::{anyhow, Result};
use nix::sys::stat::{umask, Mode};

use crate::protocol::{Reply, Request};
use crate::server::serve_one;

const SOCKET_MODE: u32 = 0o660;
/// Octal 0177: new files are readable and writable by their owner only.
const PRIVATE_UMASK: Mode = Mode::S_IXUSR.union(Mode::S_IRWXG).union(Mode::S_IRWXO);

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

/// Accept and serve connections one at a time until killed. A connection that
/// fails is logged and does not stop the loop.
pub fn run(
    listener: &UnixListener,
    deploy_uid: u32,
    handle: &mut impl FnMut(Request) -> Reply,
) -> Result<()> {
    loop {
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
