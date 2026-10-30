//! Checking that an engine answers the commands riku drives.
//!
//! Security model: only fixed command names from [`REQUIRED_COMMANDS`] are
//! passed, each as its own argv element, to the already resolved engine path.

use std::path::Path;
use std::process::Command;

use riku_util::spawn::output_retrying_etxtbsy;

/// Engine subcommands riku depends on. An engine missing any of them is refused.
pub const REQUIRED_COMMANDS: [&str; 13] = [
    "pull", "build", "run", "stop", "kill", "rm", "ps", "inspect", "stats", "logs", "version",
    "network", "volume",
];

/// Run `<program> <command> --help` for every required command.
///
/// Capability is judged by the command answering, never by the engine's name.
/// Returns the first command that failed or could not be spawned, if any.
pub fn first_unanswered(program: &Path) -> Option<&'static str> {
    REQUIRED_COMMANDS.into_iter().find(|command| {
        match output_retrying_etxtbsy(Command::new(program).args([command, "--help"])) {
            Ok(out) => !out.status.success(),
            Err(_) => true,
        }
    })
}

#[cfg(test)]
#[path = "probe_tests.rs"]
mod tests;
