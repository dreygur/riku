//! The deploy side of the router helper socket.
//!
//! Security: the helper is a root process and its answer is data, not
//! instructions. The reply is read with the same size and time bounds the
//! helper applies to requests, then parsed as one known reply; nothing in it
//! is executed or used as a path. The socket path is a constant, never taken
//! from an app's ENV.

use std::io::{ErrorKind, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

use anyhow::{anyhow, Result};
use riku_route::{
    encode_request, is_closed, parse_reply, read_bounded, Reply, Request, MAX_MESSAGE_BYTES,
};

use crate::config::RikuPaths;

/// How long a deploy waits for the helper to take the request and, separately,
/// to answer it. Other deploys queue ahead of it, and a router check and
/// reload run inside the wait.
const ANSWER_BUDGET: Duration = Duration::from_secs(60);

/// Send `request` to the helper on its socket and return its reply.
pub fn send(paths: &RikuPaths, request: &Request) -> Result<Reply> {
    send_to(&paths.router_socket, request, ANSWER_BUDGET)
}

fn send_to(socket: &Path, request: &Request, budget: Duration) -> Result<Reply> {
    let body = match encode_request(request) {
        Ok(body) => body,
        Err(e) => return Err(anyhow!("cannot send the request: {e}")),
    };
    let mut stream = match UnixStream::connect(socket) {
        Ok(stream) => stream,
        Err(e) => {
            return Err(anyhow!(
                "router helper unavailable: cannot reach it at {}: {e}. Is `riku router-helper` running?",
                socket.display()
            ))
        }
    };
    // The helper may refuse and close before it has read everything. Its
    // reason is in the reply, so a write that failed because the peer is
    // gone is only reported through the missing answer. macOS refuses to set
    // a timeout on a socket whose peer already closed, with EINVAL; that is
    // the same closed peer.
    let written = match stream.set_write_timeout(Some(budget)) {
        Ok(()) => stream
            .write_all(&body)
            .and_then(|()| stream.shutdown(Shutdown::Write)),
        Err(e) if e.kind() == ErrorKind::InvalidInput => Err(ErrorKind::BrokenPipe.into()),
        Err(e) => return Err(anyhow!("failed to set the write timeout: {e}")),
    };
    let answer = match read_bounded(&stream, MAX_MESSAGE_BYTES, budget) {
        Ok(answer) => answer,
        Err(e) => return Err(anyhow!("no answer from the router helper: {e}")),
    };
    if answer.is_empty() {
        return match written {
            Err(e) if !is_closed(e.kind()) => {
                Err(anyhow!("the router helper connection failed: {e}"))
            }
            _ => Err(anyhow!(
                "the router helper closed the connection without an answer. It only serves root and the deploy user"
            )),
        };
    }
    parse_reply(&answer)
}

#[cfg(test)]
#[path = "helper_client_tests.rs"]
mod tests;
