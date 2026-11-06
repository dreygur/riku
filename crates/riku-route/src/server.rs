//! Serves one connection of the router helper.
//!
//! Security model: this runs as root and the other end is the deploy user,
//! who is less trusted than the process. Two checks guard the socket and both
//! are required: its group lets only the deploy user's primary group connect
//! (set where the socket is created, see [`crate::listen`]), and
//! [`serve_one`] reads the connecting process's uid from the kernel with
//! `SO_PEERCRED` and drops any uid but the deploy user's and 0 without
//! answering. Input is bounded in size ([`MAX_MESSAGE_BYTES`], checked while
//! reading, before parsing) and in time ([`REQUEST_BUDGET`] in total, so a
//! slow sender cannot hold the helper). One connection is served at a time;
//! others wait in the listen backlog. A request is one object: unknown fields
//! and other protocol versions are refused, and no field becomes a path.

use std::io::Write;
use std::os::unix::net::UnixStream;
use std::time::Duration;

use anyhow::{anyhow, Result};

use crate::bounded::{read_bounded, MAX_MESSAGE_BYTES};
use crate::protocol::{encode_reply, parse_request, Reply, Request, Stage};

/// The total time a peer gets to send its request, and to take the reply.
pub const REQUEST_BUDGET: Duration = Duration::from_secs(10);

#[cfg(target_os = "linux")]
fn peer_uid(stream: &UnixStream) -> Result<u32> {
    use nix::sys::socket::{getsockopt, sockopt::PeerCredentials};
    match getsockopt(stream, PeerCredentials) {
        Ok(credentials) => Ok(credentials.uid()),
        Err(e) => Err(anyhow!("failed to read the peer credentials: {e}")),
    }
}

#[cfg(target_os = "macos")]
fn peer_uid(stream: &UnixStream) -> Result<u32> {
    use nix::sys::socket::{getsockopt, sockopt::LocalPeerCred};
    match getsockopt(stream, LocalPeerCred) {
        Ok(credentials) => Ok(credentials.uid()),
        Err(e) => Err(anyhow!("failed to read the peer credentials: {e}")),
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn peer_uid(_stream: &UnixStream) -> Result<u32> {
    Err(anyhow!("peer credentials are not available on this system"))
}

fn send_reply(mut stream: &UnixStream, reply: &Reply) -> Result<()> {
    let bytes = match encode_reply(reply) {
        Ok(bytes) => bytes,
        Err(e) => return Err(anyhow!("cannot answer the request: {e}")),
    };
    if let Err(e) = stream.set_write_timeout(Some(REQUEST_BUDGET)) {
        return Err(anyhow!("failed to set the write timeout: {e}"));
    }
    match stream.write_all(&bytes) {
        Ok(()) => Ok(()),
        Err(e) => Err(anyhow!("failed to write the reply: {e}")),
    }
}

fn refuse_request(stream: &UnixStream, reason: &anyhow::Error) -> Result<()> {
    send_reply(
        stream,
        &Reply::Failed {
            stage: Stage::Schema,
            error: reason.to_string(),
        },
    )
}

fn read_request(stream: &UnixStream) -> Result<Request> {
    let body = match read_bounded(stream, MAX_MESSAGE_BYTES, REQUEST_BUDGET) {
        Ok(body) => body,
        Err(e) => return Err(anyhow!("request not read: {e}")),
    };
    parse_request(&body)
}

/// Serve the one request on `stream` and close it.
///
/// A peer whose uid is neither `deploy_uid` nor 0 gets no answer and an
/// error. A request that is too large, too slow or malformed gets a
/// [`Stage::Schema`] failure. Everything else goes to `handle`.
pub fn serve_one(
    stream: UnixStream,
    deploy_uid: u32,
    handle: &mut impl FnMut(Request) -> Reply,
) -> Result<()> {
    let peer = match peer_uid(&stream) {
        Ok(peer) => peer,
        Err(e) => return Err(anyhow!("cannot identify the peer: {e}")),
    };
    if peer != 0 && peer != deploy_uid {
        return Err(anyhow!("refused a connection from uid {peer}"));
    }
    match read_request(&stream) {
        Ok(request) => send_reply(&stream, &handle(request)),
        Err(e) => refuse_request(&stream, &e),
    }
}

#[cfg(test)]
#[path = "server_tests.rs"]
mod tests;
