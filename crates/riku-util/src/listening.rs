//! Ports something on this host already listens on, from `/proc/net/tcp`
//! and `/proc/net/tcp6`.
//!
//! Port mode refuses these, which covers sshd wherever it is configured,
//! databases, pinned app ports and anything else riku does not manage. Off
//! Linux the files are missing and the set is empty.

use std::collections::BTreeSet;
use std::fs;

const PROC_TABLES: [&str; 2] = ["/proc/net/tcp", "/proc/net/tcp6"];
/// The `st` column value for a listening socket.
const LISTEN_STATE: &str = "0A";

fn listen_port(row: &str) -> Option<u16> {
    let mut fields = row.split_whitespace();
    let local = fields.nth(1);
    let state = fields.nth(1);
    match (local, state) {
        (Some(local), Some(LISTEN_STATE)) => match local.rsplit_once(':') {
            Some((_, hex)) => u16::from_str_radix(hex, 16).ok(),
            None => None,
        },
        _ => None,
    }
}

/// The listening ports in one `/proc/net/tcp` style table.
pub fn listen_ports_in(table: &str) -> BTreeSet<u16> {
    table.lines().skip(1).filter_map(listen_port).collect()
}

/// Every TCP port in LISTEN on this host, IPv4 and IPv6.
pub fn listening_ports() -> BTreeSet<u16> {
    PROC_TABLES
        .iter()
        .filter_map(|table| fs::read_to_string(table).ok())
        .flat_map(|text| listen_ports_in(&text))
        .collect()
}

#[cfg(test)]
#[path = "listening_tests.rs"]
mod tests;
