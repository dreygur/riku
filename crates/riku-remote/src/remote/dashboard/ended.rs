//! The error shown when the ssh client running the remote dashboard exits.

use std::process::ExitStatus;

/// ssh exits with this status when the connection fails or drops.
const SSH_CONNECTION_ERROR: i32 = 255;

/// What to tell the user once the client for `target` has exited as `status`.
pub(super) fn ended_message(target: &str, status: &ExitStatus) -> String {
    if status.code() == Some(SSH_CONNECTION_ERROR) {
        return format!("the ssh connection to {target} dropped or could not be made");
    }
    format!("the remote dashboard ended ({status})")
}

#[cfg(test)]
#[path = "ended_tests.rs"]
mod tests;
