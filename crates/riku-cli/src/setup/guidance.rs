//! Post-init user-facing output: verification summary and first-app guide.

use std::path::PathBuf;
use std::process::Command;

use crate::util::echo;

use super::binary::INSTALL_DIR;

const DOCS_URL: &str = "https://dreygur.github.io/riku/";
const SYSTEM_SERVICE_CHECK: [&str; 2] = ["is-active", "riku"];
const USER_SERVICE_CHECK: [&str; 3] = ["--user", "is-active", "riku"];

/// Whose install the summary checks. A root init sets up the deploy
/// user's tree, so checking root's own home or `--user` service there
/// reports a working install as broken.
pub(super) enum InstallScope {
    /// A root init: the deploy user's tree and the system `riku.service`.
    System { user: String, home: PathBuf },
    /// A per-user init: the caller's tree and their `--user` service.
    User { user: String, home: PathBuf },
}

impl InstallScope {
    fn user(&self) -> &str {
        match self {
            Self::System { user, .. } | Self::User { user, .. } => user,
        }
    }

    fn binary(&self) -> PathBuf {
        match self {
            Self::System { home, .. } | Self::User { home, .. } => {
                home.join(INSTALL_DIR).join("riku")
            }
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
    let binary = scope.binary();
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

fn remote_line(user: &str) -> String {
    format!("   git remote add riku {user}@your-server:myapp")
}

fn print_first_app_guide(user: &str) {
    echo("Deploy your first app:", "green");
    echo("", "");
    echo("1. Create an app directory on your local machine:", "");
    echo("   mkdir myapp && cd myapp", "");
    echo("   git init", "");
    echo("", "");
    echo("2. Add your code and a Procfile:", "");
    echo("   echo 'web: python app.py' > Procfile", "");
    echo("", "");
    echo("3. Deploy:", "");
    echo(&remote_line(user), "");
    echo("   git push riku main", "");
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
