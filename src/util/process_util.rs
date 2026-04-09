//! Process and binary utilities.

use anyhow::Result;
use regex::Regex;
use std::net::TcpListener;
use std::process::Command;
use which::which;

use super::display::echo;

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

/// Run shell command, return stdout. Return empty string on failure.
#[allow(dead_code)]
pub fn command_output(cmd: &str) -> String {
    match Command::new("sh").arg("-c").arg(cmd).output() {
        Ok(output) => String::from_utf8_lossy(&output.stdout).to_string(),
        Err(_) => String::new(),
    }
}

/// Check all binaries exist via `which`. Print results.
#[allow(dead_code)]
pub fn check_requirements(binaries: &[&str]) -> bool {
    echo(
        &format!("-----> Checking requirements: {:?}", binaries),
        "green",
    );
    let results: Vec<Option<std::path::PathBuf>> = binaries.iter().map(|b| which(b).ok()).collect();
    echo(&format!("{:?}", results), "");

    results.iter().all(|r| r.is_some())
}

/// Validate a Node.js version string.
#[allow(dead_code)]
pub fn validate_node_version(version: &str) -> Result<(), String> {
    let version = version.trim();

    if version.is_empty() {
        return Err("NODE_VERSION cannot be empty".to_string());
    }

    // Basic version format check (e.g., "18.17.0", "18", "18.x")
    let version_regex = Regex::new(r"^\d+(\.\d+)*(-[\w.]+)?$").unwrap();
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
    fn test_check_requirements_existing() {
        assert!(check_requirements(&["sh"]));
    }

    #[test]
    fn test_check_requirements_missing() {
        assert!(!check_requirements(&["nonexistent_binary_xyz"]));
    }

    #[test]
    fn test_command_output_success() {
        let output = command_output("echo hello");
        assert_eq!(output.trim(), "hello");
    }

    #[test]
    fn test_command_output_failure() {
        let output = command_output("nonexistent_command_xyz 2>/dev/null");
        assert!(output.is_empty() || output.contains("not found"));
    }

    #[test]
    fn test_validate_node_version_valid() {
        assert!(validate_node_version("18.17.0").is_ok());
        assert!(validate_node_version("18").is_ok());
        assert!(validate_node_version("20.0.0").is_ok());
        assert!(validate_node_version("18.17").is_ok());
    }

    #[test]
    fn test_validate_node_version_invalid() {
        assert!(validate_node_version("").is_err());
        assert!(validate_node_version("abc").is_err());
        assert!(validate_node_version("18.17").is_ok()); // This is actually valid
    }
}
