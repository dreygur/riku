//! Host port allocation for routed copies.

use std::net::TcpListener;

use anyhow::{anyhow, Result};

/// A loopback TCP port nothing listens on right now.
///
/// The port is released before it is returned, so another process could take
/// it before the container binds it; the engine then fails to start the copy
/// and the caller sees the error.
pub fn pick_free_port() -> Result<u16> {
    match hold_free_port() {
        Ok((_listener, port)) => Ok(port),
        Err(e) => Err(e),
    }
}

fn hold_free_port() -> Result<(TcpListener, u16)> {
    let listener = match TcpListener::bind(("127.0.0.1", 0)) {
        Ok(listener) => listener,
        Err(e) => return Err(anyhow!("cannot find a free port: {e}")),
    };
    match listener.local_addr() {
        Ok(addr) => Ok((listener, addr.port())),
        Err(e) => Err(anyhow!("cannot read the free port: {e}")),
    }
}

#[cfg(test)]
#[path = "ports_tests.rs"]
mod tests;
