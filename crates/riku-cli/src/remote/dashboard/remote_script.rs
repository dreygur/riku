//! The shell script a remote dashboard session runs, and the text it sends
//! and reads back.
//!
//! Security model: nothing secret or user supplied is spliced into script
//! text. The deploy user reaches the script as `$1`, and the token, plus the
//! sudo password when one is needed, travel on the session's stdin. The
//! token is exported inside `sudo -u`, so sudo's env_reset cannot strip it
//! and it never shows in an argument list on the server.

/// Start of the line `riku dashboard` prints once it has bound its port.
pub const BOUND_PREFIX: &str = "riku-dashboard-bound ";

const SCRIPT_BODY: &str = " -H -u \"$1\" sh -c '
  IFS= read -r t
  export RIKU_DASHBOARD_TOKEN=\"$t\"
  bin=$(command -v riku || echo \"$HOME/.local/bin/riku\")
  \"$bin\" dashboard --bind 127.0.0.1:0 &
  p=$!
  cat >/dev/null
  kill \"$p\"
'";

/// `$0` for the outer `sh -c`, which shows up in error messages.
const SCRIPT_NAME: &str = "riku-dashboard";

const PASSWORD_REQUIRED: &str = "a password is required";

/// How sudo on the server gets permission to switch to the deploy user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SudoMode {
    /// NOPASSWD rule or cached credential: `sudo -n`.
    NoPassword,
    /// Password read from the first stdin line: `sudo -S -p ''`.
    Password,
}

impl SudoMode {
    fn flags(self) -> &'static str {
        match self {
            SudoMode::NoPassword => "-n",
            SudoMode::Password => "-S -p ''",
        }
    }
}

/// The script text for `mode`. Expects the deploy user as `$1`.
pub fn script(mode: SudoMode) -> String {
    format!("sudo {}{SCRIPT_BODY}", mode.flags())
}

/// Remote command words that run the script as `deploy`.
pub fn script_command(mode: SudoMode, deploy: &str) -> Vec<String> {
    vec![
        "sh".to_string(),
        "-c".to_string(),
        script(mode),
        SCRIPT_NAME.to_string(),
        deploy.to_string(),
    ]
}

/// Remote command words that ask sudo, without prompting, whether the admin
/// may act as `deploy`.
pub fn probe_command(deploy: &str) -> Vec<String> {
    ["sudo", "-n", "-u", deploy, "true"]
        .iter()
        .map(|word| word.to_string())
        .collect()
}

/// Whether the probe failed only because sudo wants a password.
pub fn needs_password(probe_stderr: &str) -> bool {
    probe_stderr.contains(PASSWORD_REQUIRED)
}

/// What the session writes to the script's stdin: the password line when
/// sudo reads one, then the token line.
pub fn payload(password: Option<&str>, token: &str) -> String {
    match password {
        Some(password) => format!("{password}\n{token}\n"),
        None => format!("{token}\n"),
    }
}

/// The port from a `riku-dashboard-bound <port>` line.
pub fn parse_bound(line: &str) -> Option<u16> {
    match line.trim_end().strip_prefix(BOUND_PREFIX) {
        Some(port) => port.parse().ok(),
        None => None,
    }
}

#[cfg(test)]
#[path = "remote_script_tests.rs"]
mod tests;
