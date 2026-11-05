//! Reads one message from a unix socket with a hard size and time budget.
//!
//! Both ends use it: the helper so a deploy user cannot make a root process
//! buffer without limit or wait on it forever, the client so a stuck helper
//! cannot hang a deploy.

use std::io::{ErrorKind, Read};
use std::os::fd::AsFd;
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use nix::errno::Errno;
use nix::poll::{poll, PollFd, PollFlags, PollTimeout};

/// The most bytes a request or reply may carry.
pub const MAX_MESSAGE_BYTES: usize = 1024 * 1024;

const CHUNK_BYTES: usize = 8 * 1024;

/// Read until the peer closes its write side. Fails when more than `limit`
/// bytes arrive, or when `budget` elapses in total, however slowly the peer
/// sends. The size is checked as bytes arrive, before anything is parsed.
pub fn read_bounded(stream: &UnixStream, limit: usize, budget: Duration) -> Result<Vec<u8>> {
    let deadline = Instant::now() + budget;
    let mut reader = stream;
    let mut message = Vec::new();
    let mut chunk = [0u8; CHUNK_BYTES];
    loop {
        match wait_readable(stream, deadline) {
            Ok(true) => {}
            Ok(false) => {
                return Err(anyhow!(
                    "timed out after {budget:?} waiting for the message"
                ))
            }
            Err(e) => return Err(anyhow!("failed to wait for the message: {e}")),
        }
        match reader.read(&mut chunk) {
            Ok(0) => return Ok(message),
            Ok(count) => {
                message.extend_from_slice(&chunk[..count]);
                if message.len() > limit {
                    return Err(anyhow!("message is larger than {limit} bytes"));
                }
            }
            Err(e) if e.kind() == ErrorKind::Interrupted => {}
            Err(e) if is_closed(e.kind()) => return Ok(message),
            Err(e) => return Err(anyhow!("failed to read the message: {e}")),
        }
    }
}

/// Whether an I/O error means the peer is gone. Linux reports a peer that
/// closed with unread data as a reset rather than end of file, and macOS
/// reports a shutdown on a socket the peer already closed as not connected.
pub fn is_closed(kind: ErrorKind) -> bool {
    matches!(
        kind,
        ErrorKind::ConnectionReset | ErrorKind::BrokenPipe | ErrorKind::NotConnected
    )
}

/// Wait until `stream` has data or is closed. `false` means the deadline
/// passed first. This polls instead of setting a socket timeout, which some
/// systems refuse once the peer has closed.
fn wait_readable(stream: &UnixStream, deadline: Instant) -> Result<bool> {
    loop {
        let remaining = match deadline.checked_duration_since(Instant::now()) {
            Some(left) if !left.is_zero() => left,
            _ => return Ok(false),
        };
        let timeout = PollTimeout::try_from(remaining).unwrap_or(PollTimeout::MAX);
        let mut fds = [PollFd::new(stream.as_fd(), PollFlags::POLLIN)];
        match poll(&mut fds, timeout) {
            Ok(ready) => return Ok(ready > 0),
            Err(Errno::EINTR) => {}
            Err(e) => return Err(anyhow!("failed to wait for the message: {e}")),
        }
    }
}
