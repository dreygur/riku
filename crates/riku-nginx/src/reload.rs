//! Direct nginx reload (`nginx -s reload`).
//!
//! The reload is graceful: in-flight connections finish on the old workers
//! while new ones pick up the refreshed config. The program and its
//! arguments are fixed, nothing from an app reaches the command line.

use std::ffi::OsStr;
use std::process::Command;

use anyhow::{anyhow, Result};

/// Ask the nginx master run by `program` to reload its configuration.
///
/// Fails when nginx is missing, not running, or rejects the config, with
/// nginx's own message in the error.
pub fn reload_nginx_with(program: impl AsRef<OsStr>) -> Result<()> {
    let output = match crate::util::spawn::output_retrying_etxtbsy(
        Command::new(program.as_ref()).args(["-s", "reload"]),
    ) {
        Ok(output) => output,
        Err(e) => return Err(anyhow!("could not run nginx: {e}")),
    };
    if output.status.success() {
        return Ok(());
    }
    Err(anyhow!(
        "nginx reload failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    ))
}

#[cfg(test)]
#[path = "reload_tests.rs"]
mod tests;
