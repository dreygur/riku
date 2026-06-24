//! Post-init user-facing output: verification summary and first-app guide.

use std::env;
use std::path::PathBuf;
use std::process::Command;

use crate::util::echo;

/// Check whether the supervisor is running and print the result.
pub(super) fn verify_supervisor(no_systemd: bool) {
    if !no_systemd {
        let status = Command::new("systemctl")
            .args(["--user", "is-active", "riku"])
            .output();

        if let Ok(output) = status {
            if output.status.success() {
                echo("      ✓ Supervisor running", "green");
            } else {
                echo(
                    "      ⚠ Supervisor not running (start with: systemctl --user start riku)",
                    "yellow",
                );
            }
        }
    } else {
        echo(
            "      ℹ Supervisor not started (start manually with: riku supervisor)",
            "yellow",
        );
    }
}

/// Print a verification summary after initialization.
pub(super) fn print_verification_summary(no_systemd: bool) {
    echo("Verification:", "green");

    if let Ok(home) = env::var("HOME") {
        let riku_path = PathBuf::from(&home).join(".local/bin/riku");
        if riku_path.exists() {
            echo(
                &format!("  ✓ Binary installed: {}", riku_path.display()),
                "green",
            );
        } else {
            echo(
                &format!("  ⚠ Binary not found: {}", riku_path.display()),
                "yellow",
            );
        }
    }

    if !no_systemd {
        let status = Command::new("systemctl")
            .args(["--user", "is-active", "riku"])
            .output();

        if let Ok(output) = status {
            if output.status.success() {
                echo("  ✓ Supervisor running", "green");
            } else {
                echo(
                    "  ⚠ Supervisor not running (start with: systemctl --user start riku)",
                    "yellow",
                );
            }
        }
    } else {
        echo(
            "  ℹ Supervisor not started (start manually with: riku supervisor)",
            "yellow",
        );
    }

    echo("", "");
}

/// Print the "deploy your first app" quick-start guide.
pub(super) fn print_first_app_guide() {
    echo("Deploy your first app:", "green");
    echo("", "");
    echo("1. Create app directory on your local machine:", "yellow");
    echo("   mkdir myapp && cd myapp", "yellow");
    echo("   git init", "yellow");
    echo("", "");
    echo("2. Add your code and create a Procfile:", "yellow");
    echo("   echo 'web: python app.py' > Procfile", "yellow");
    echo("", "");
    echo("3. Deploy:", "yellow");
    echo(
        &format!(
            "   git remote add riku {}@your-server:myapp",
            env::var("USER").unwrap_or_else(|_| "deploy".to_string())
        ),
        "yellow",
    );
    echo("   git push riku main", "yellow");
    echo("", "");
    echo("Documentation: https://dreygur.github.io/riku/", "green");
    echo("", "");
}
