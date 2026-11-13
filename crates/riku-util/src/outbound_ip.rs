//! The address this host uses for outbound traffic.
//!
//! A UDP socket is "connected" to a documentation address and the local
//! address the kernel chose is read back. No packet is sent. The result may
//! be private; callers that need a public address check it with
//! [`public_v4`].

use std::net::{IpAddr, Ipv4Addr, UdpSocket};

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

fn shared_address_space(ip: Ipv4Addr) -> bool {
    let [first, second, ..] = ip.octets();
    first == 100 && (64..128).contains(&second)
}

/// `ip` when it is an IPv4 address reachable from the internet: not private,
/// loopback, link-local, unspecified, broadcast or carrier-grade NAT.
pub fn public_v4(ip: IpAddr) -> Option<Ipv4Addr> {
    let IpAddr::V4(v4) = ip else {
        return None;
    };
    let local = v4.is_private()
        || v4.is_loopback()
        || v4.is_link_local()
        || v4.is_unspecified()
        || v4.is_broadcast()
        || shared_address_space(v4);
    if local {
        return None;
    }
    Some(v4)
}
