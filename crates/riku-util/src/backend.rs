//! Where a router sends an app's traffic.

use std::fmt;
use std::net::{IpAddr, Ipv4Addr};
use std::path::PathBuf;

/// One upstream a router forwards to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Backend {
    /// A TCP port on `host`, usually the loopback interface.
    Tcp { host: IpAddr, port: u16 },
    /// A unix socket at this path.
    Unix(PathBuf),
}

impl Backend {
    /// A TCP backend on `127.0.0.1`.
    pub fn loopback(port: u16) -> Self {
        Backend::Tcp {
            host: IpAddr::V4(Ipv4Addr::LOCALHOST),
            port,
        }
    }
}

impl fmt::Display for Backend {
    /// Renders the address as routers expect it: `<host>:<port>` (IPv6 hosts
    /// in brackets, `[::1]:5000`) or `unix:<path>`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Backend::Tcp {
                host: IpAddr::V6(host),
                port,
            } => write!(f, "[{host}]:{port}"),
            Backend::Tcp { host, port } => write!(f, "{host}:{port}"),
            Backend::Unix(path) => write!(f, "unix:{}", path.display()),
        }
    }
}

#[cfg(test)]
#[path = "backend_tests.rs"]
mod tests;
