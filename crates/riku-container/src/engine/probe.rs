//! Checking that an engine answers the commands riku drives.
//!
//! Security model: only fixed command names from [`REQUIRED_COMMANDS`] are
//! passed, each as its own argv element, to the already resolved engine path.
//! Each probe is time bounded like every other engine call.

use std::ffi::OsString;
use std::path::Path;
use std::time::Duration;

use super::bounded::capture;

/// Engine subcommands riku depends on. An engine missing any of them is refused.
pub const REQUIRED_COMMANDS: [&str; 16] = [
    "pull", "build", "run", "stop", "kill", "rm", "ps", "inspect", "stats", "logs", "version",
    "network", "volume", "image", "login", "wait",
];

/// Run `<program> <command> --help` for every required command, each within
/// `limit`.
///
/// Capability is judged by the command answering, never by the engine's name.
/// Returns the first command that failed, timed out or could not be spawned,
/// with the reason: the spawn or timeout error, or the exit status plus
/// trimmed stderr.
pub fn first_unanswered(program: &Path, limit: Duration) -> Option<(&'static str, String)> {
    for command in REQUIRED_COMMANDS {
        let args = [OsString::from(command), OsString::from("--help")];
        match capture(program, &args, limit) {
            Ok(out) if out.status.success() => {}
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                return Some((command, format!("{}: {}", out.status, stderr.trim())));
            }
            Err(e) => return Some((command, e.to_string())),
        }
    }
    None
}

#[cfg(test)]
#[path = "probe_tests.rs"]
mod tests;
