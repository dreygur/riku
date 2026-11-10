//! Post-init user-facing output: verification summary and first-app guide.

use std::path::Path;
use std::process::Command;

use crate::util::{echo, ROOT_BINARY};

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

fn print_summary(scope: &InstallScope, no_systemd: bool) {
    echo("[4/4] Verifying installation...", "");
    let binary = Path::new(ROOT_BINARY);
    if binary.exists() {
        echo(
            &format!("      ✓ Binary installed: {}", binary.display()),
            "",
        );
    } else {
        echo(&format!("Binary not found: {}", binary.display()), "yellow");
    }

    if no_systemd {
        echo(
            "      Supervisor not started (start it with: riku supervisor)",
            "",
        );
    } else if supervisor_running(scope) {
        echo("      ✓ Supervisor running", "");
    } else {
        echo(
            &format!(
                "Supervisor not running (start it with: {})",
                scope.start_hint()
            ),
            "yellow",
        );
    }
    echo("", "");
}

/// The steps after init, run on the developer's laptop. riku cannot know
/// the name the laptop reaches this host by, so `<host>` stays for them.
fn first_app_guide(user: &str) -> Vec<String> {
    vec![
        "1. On your laptop, inside your app's git repo, register this server:".to_string(),
        format!("   rk server add <name> {user}@<host> --admin <login>@<host>"),
        "   It authorizes your ssh key and adds the riku git remote.".to_string(),
        String::new(),
        "2. Deploy:".to_string(),
        "   git push riku main".to_string(),
        String::new(),
        "Processes come from riku.toml or a Procfile, else the runtime's start command."
            .to_string(),
    ]
}

fn print_first_app_guide(user: &str) {
    echo("Deploy your first app:", "green");
    echo("", "");
    for line in first_app_guide(user) {
        echo(&line, "");
    }
    echo("", "");
    echo(&format!("Documentation: {DOCS_URL}"), "");
}

/// Print the verification summary and the first app guide after an init.
pub(super) fn print_finish(scope: &InstallScope, no_systemd: bool) {
    print_summary(scope, no_systemd);
    echo("Riku server initialized successfully!", "green");
    echo("", "");
    print_first_app_guide(scope.user());
}

#[cfg(test)]
#[path = "guidance_tests.rs"]
mod tests;
