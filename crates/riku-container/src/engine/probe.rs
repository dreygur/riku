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
pub const REQUIRED_COMMANDS: [&str; 17] = [
    "pull", "build", "run", "stop", "kill", "rm", "ps", "inspect", "stats", "logs", "version",
    "network", "volume", "image", "login", "wait", "info",
];

/// Run `<program> --version` within `limit` and return whether it exited 0.
pub fn answers_version(program: &Path, env: &[(String, String)], limit: Duration) -> bool {
    exits_zero_within(program, env, &[OsString::from("--version")], limit)
}

/// Run `program` with `args` and `env` within `limit` and return whether it
/// exited 0. A spawn failure or timeout counts as a non zero exit.
pub fn exits_zero_within(
    program: &Path,
    env: &[(String, String)],
    args: &[OsString],
    limit: Duration,
) -> bool {
    match capture(program, env, args, limit) {
        Ok(out) => out.status.success(),
        Err(_) => false,
    }
}

/// Run `<program> <command> --help` for every required command, each within
/// `limit`, with `env` set.
///
/// Capability is judged by the command answering, never by the engine's name.
/// Returns the first command, in [`REQUIRED_COMMANDS`] order, that failed,
/// timed out or could not be spawned, with the reason: the spawn or timeout
/// error, or the exit status plus trimmed stderr.
///
/// The probes run at the same time. Each one is a full client start, and run
/// one after another the seventeen of them add a few hundred milliseconds to
/// every engine open, which `riku doctor` and the dashboard state read do.
pub fn first_unanswered(
    program: &Path,
    env: &[(String, String)],
    limit: Duration,
) -> Option<(&'static str, String)> {
    let failures: Vec<Option<String>> = std::thread::scope(|scope| {
        let probes: Vec<_> = REQUIRED_COMMANDS
            .iter()
            .map(|command| scope.spawn(move || unanswered_reason(program, env, command, limit)))
            .collect();
        probes
            .into_iter()
            .map(|probe| match probe.join() {
                Ok(reason) => reason,
                Err(_) => Some("probe thread panicked".to_string()),
            })
            .collect()
    });
    REQUIRED_COMMANDS
        .into_iter()
        .zip(failures)
        .find_map(|(command, failure)| failure.map(|reason| (command, reason)))
}

/// Why `<program> <command> --help` did not answer, or `None` when it exited 0.
fn unanswered_reason(
    program: &Path,
    env: &[(String, String)],
    command: &str,
    limit: Duration,
) -> Option<String> {
    let args = [OsString::from(command), OsString::from("--help")];
    match capture(program, env, &args, limit) {
        Ok(out) if out.status.success() => None,
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            Some(format!("{}: {}", out.status, stderr.trim()))
        }
        Err(e) => Some(e.to_string()),
    }
}

#[cfg(test)]
#[path = "probe_tests.rs"]
mod tests;
