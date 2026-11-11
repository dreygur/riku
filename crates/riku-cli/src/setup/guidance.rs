//! The end of `riku init`: the supervisor check, the outro and the summary
//! that says what to run next.

use std::path::Path;
use std::process::Command;

use crate::util::{display, ROOT_BINARY};

const DOCS_URL: &str = "https://dreygur.github.io/riku/";
const SYSTEM_SERVICE_CHECK: [&str; 2] = ["is-active", "riku"];
const USER_SERVICE_CHECK: [&str; 3] = ["--user", "is-active", "riku"];

/// Whose install the summary checks. A root init sets up the deploy
/// user's tree, so checking root's own `--user` service there reports a
/// working install as broken.
pub(super) enum InstallScope {
    /// A root init: the deploy user and the system `riku.service`.
    System { user: String },
    /// A per-user init: the caller and their `--user` service.
    User { user: String },
}

impl InstallScope {
    fn user(&self) -> &str {
        match self {
            Self::System { user } | Self::User { user } => user,
        }
    }

    fn systemctl_args(&self) -> &'static [&'static str] {
        match self {
            Self::System { .. } => &SYSTEM_SERVICE_CHECK,
            Self::User { .. } => &USER_SERVICE_CHECK,
        }
    }

    fn start_hint(&self) -> &'static str {
        match self {
            Self::System { .. } => "sudo systemctl start riku",
            Self::User { .. } => "systemctl --user start riku",
        }
    }
}

fn supervisor_running(scope: &InstallScope) -> bool {
    Command::new("systemctl")
        .args(scope.systemctl_args())
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn check_supervisor(scope: &InstallScope, no_systemd: bool) {
    if no_systemd {
        display::step_done(
            "Supervisor",
            "not started (--no-systemd), run: riku supervisor",
        );
    } else if supervisor_running(scope) {
        display::step_done("Supervisor", "running");
    } else {
        display::warn(&format!(
            "Supervisor not running, start it with: {}",
            scope.start_hint()
        ));
    }
}

/// The summary after the outro: what was set up and what the developer
/// runs next on the laptop (`--admin` authorizes their key for pushes).
/// riku cannot know the name the laptop reaches this host by, so `<host>`
/// stays for them.
fn finish_summary(user: &str, router: &str) -> Vec<(&'static str, String)> {
    vec![
        ("Router", router.to_string()),
        ("Binary", ROOT_BINARY.to_string()),
        (
            "Next",
            format!(
                "rk server add <name> {user}@<host> --admin <login>@<host>, then git push riku main"
            ),
        ),
        ("Docs", DOCS_URL.to_string()),
    ]
}

/// Check the supervisor, then print the outro and the summary.
pub(super) fn print_finish(scope: &InstallScope, no_systemd: bool, router: &str) {
    if !Path::new(ROOT_BINARY).exists() {
        display::warn(&format!("Binary not found: {ROOT_BINARY}"));
    }
    check_supervisor(scope, no_systemd);
    display::outro("Riku is ready");
    let rows = finish_summary(scope.user(), router);
    let pairs: Vec<(&str, &str)> = rows.iter().map(|(k, v)| (*k, v.as_str())).collect();
    display::summary(&pairs);
}

#[cfg(test)]
#[path = "guidance_tests.rs"]
mod tests;
