//! Where a router sends an app's traffic.

use std::fmt;
use std::path::PathBuf;

/// One upstream a router forwards to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Backend {
    /// A TCP port on the loopback interface.
    Tcp(u16),
    /// A unix socket at this path.
    Unix(PathBuf),
}

impl fmt::Display for Backend {
    /// Renders the address as routers expect it: `127.0.0.1:<port>` or
    /// `unix:<path>`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Backend::Tcp(port) => write!(f, "127.0.0.1:{port}"),
            Backend::Unix(path) => write!(f, "unix:{}", path.display()),
        }
    }
}

#[cfg(test)]
#[path = "backend_tests.rs"]
mod tests;
