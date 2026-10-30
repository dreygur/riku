//! The shell script a remote dashboard session runs, and the markers it
//! prints for the local side.
//!
//! Security model: nothing secret or user supplied is spliced into script
//! text. The deploy user reaches the script as `$1`. The sudo password, when
//! sudo asks, and then the token travel on the session's stdin. The inner
//! shell prints `READY_MARKER` only once sudo has switched user, and reads
//! the token after that, so a password can never be taken as the token. The
//! token is exported inside `sudo -u`, so sudo's env_reset cannot strip it
//! and it never shows in an argument list on the server. `--local` keeps
//! the server side dashboard from resolving a target of its own.

/// Start of the line `riku dashboard` prints once it has bound its port.
pub const BOUND_PREFIX: &str = "riku-dashboard-bound ";

/// What sudo prints on stderr, with no newline, when it wants the password.
pub(super) const SUDO_PROMPT: &str = "riku-sudo-prompt:";

/// The stdout line the inner shell prints once sudo let it run.
pub(super) const READY_MARKER: &str = "riku-ready";

/// `$0` for the outer `sh -c`, which shows up in error messages.
const SCRIPT_NAME: &str = "riku-dashboard";

/// The script text. Expects the deploy user as `$1`.
pub(super) fn script() -> String {
    format!(
        "sudo -S -p '{SUDO_PROMPT}' -H -u \"$1\" sh -c '
  echo {READY_MARKER}
  IFS= read -r t || exit 1
  export RIKU_DASHBOARD_TOKEN=\"$t\"
  bin=$(command -v riku || echo \"$HOME/.local/bin/riku\")
  \"$bin\" dashboard --local --bind 127.0.0.1:0 &
  p=$!
  cat >/dev/null
  kill \"$p\"
'"
    )
}

/// Remote command words that run the script as `deploy`.
pub(super) fn script_command(deploy: &str) -> Vec<String> {
    vec![
        "sh".to_string(),
        "-c".to_string(),
        script(),
        SCRIPT_NAME.to_string(),
        deploy.to_string(),
    ]
}

/// The port from a `riku-dashboard-bound <port>` line.
pub(super) fn parse_bound(line: &str) -> Option<u16> {
    match line.trim_end().strip_prefix(BOUND_PREFIX) {
        Some(port) => port.parse().ok(),
        None => None,
    }
}

#[cfg(test)]
#[path = "remote_script_tests.rs"]
mod tests;
