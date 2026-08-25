//! Process and binary utilities.

use anyhow::Result;
use regex::Regex;
use std::net::TcpListener;

/// Find a free TCP port (entirely at random) by binding to port 0.
/// Returns an error if no port is available.
pub fn get_free_port(address: &str) -> Result<u16> {
    let bind_addr = format!("{}:0", address);
    let listener = TcpListener::bind(&bind_addr)
        .map_err(|e| anyhow::anyhow!("Failed to bind to {}: {}", bind_addr, e))?;
    let addr = listener
        .local_addr()
        .map_err(|e| anyhow::anyhow!("Failed to get local address: {}", e))?;
    Ok(addr.port())
}

/// Validate a Node.js version string.
pub fn validate_node_version(version: &str) -> Result<(), String> {
    let version = version.trim();

    if version.is_empty() {
        return Err("NODE_VERSION cannot be empty".to_string());
    }

    // Accepts "18", "18.17.0", "18.x", "18.17.0-nightly". The value is only
    // format-checked to warn on typos: no runtime reads it (docs/docs/env.md),
    // so an npm-style wildcard segment must not draw a warning.
    let version_regex = Regex::new(r"^\d+(\.(\d+|[xX*]))*(-[\w.]+)?$").unwrap();
    if !version_regex.is_match(version) {
        return Err(format!(
            "Invalid NODE_VERSION: '{}' - expected format like '18.17.0' or '18'",
            version
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_free_port_returns_valid_port() {
        let port = get_free_port("127.0.0.1");
        assert!(port.is_ok());
        assert!(port.unwrap() > 0);
    }

    #[test]
    fn test_get_free_port_different_calls() {
        let port1 = get_free_port("127.0.0.1");
        let port2 = get_free_port("127.0.0.1");
        assert!(port1.is_ok());
        assert!(port2.is_ok());
        assert!(port1.unwrap() > 0);
        assert!(port2.unwrap() > 0);
    }

    #[test]
    fn test_validate_node_version_valid() {
        assert!(validate_node_version("18.17.0").is_ok());
        assert!(validate_node_version("18").is_ok());
        assert!(validate_node_version("20.0.0").is_ok());
        assert!(validate_node_version("18.17").is_ok());
    }

    #[test]
    fn accepts_an_npm_style_wildcard_segment() {
        assert!(validate_node_version("18.x").is_ok());
        assert!(validate_node_version("18.17.x").is_ok());
    }

    #[test]
    fn test_validate_node_version_invalid() {
        assert!(validate_node_version("").is_err());
        assert!(validate_node_version("abc").is_err());
        assert!(validate_node_version("x.18").is_err());
        assert!(validate_node_version("18.y").is_err());
    }
}
