//! The address this host uses for outbound traffic.
//!
//! A UDP socket is "connected" to a documentation address and the local
//! address the kernel chose is read back. No packet is sent. The result may
//! be private; callers that need a public address check it themselves.

use std::net::{IpAddr, UdpSocket};

/// TEST-NET-1 (RFC 5737). Only its route matters; nothing is sent to it.
const PROBE_ADDRESS: &str = "192.0.2.1:9";
const UNSPECIFIED_LOCAL: &str = "0.0.0.0:0";

/// The address this host would use for outbound traffic, if any.
pub fn detect_outbound_ip() -> Option<IpAddr> {
    let socket = match UdpSocket::bind(UNSPECIFIED_LOCAL) {
        Ok(socket) => socket,
        Err(_) => return None,
    };
    if socket.connect(PROBE_ADDRESS).is_err() {
        return None;
    }
    socket.local_addr().ok().map(|addr| addr.ip())
}
