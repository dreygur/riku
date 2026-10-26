//! Pre-flight checks for `riku init`: root warning and missing tools.

use std::process::Command;

use crate::config::router::Router;
use crate::util::echo;

/// The router's display name, binary and version flag, `None` for a
/// plugin router.
fn router_program(router: &Router) -> Option<(&'static str, &'static str, &'static str)> {
    match router {
        Router::Nginx => Some(("Nginx", "nginx", "-v")),
        Router::Caddy => Some(("Caddy", "caddy", "version")),
        Router::Plugin(_) => None,
    }
}

/// Tell a non-root caller what a root init would also set up.
pub fn warn_not_root(router: &Router) {
    echo("⚠ Warning: Not running as root", "yellow");
    echo("  Some features require root privileges:", "yellow");
    echo("    - System-wide systemd service", "yellow");
    if let Some((label, _, _)) = router_program(router) {
        echo(&format!("    - {label} configuration"), "yellow");
    }
    echo("  Run 'sudo riku init' for full installation.", "yellow");
    echo("", "");
}

/// Report missing git, and a missing router binary when running as root.
pub fn check_prerequisites(is_root: bool, router: &Router) {
    echo("Checking prerequisites...", "");
    let mut missing_deps = Vec::new();

    if Command::new("git").arg("--version").output().is_err() {
        missing_deps.push("git");
    }

    if let Some((_, program, version_flag)) = router_program(router) {
        let installed = Command::new(program).arg(version_flag).output().is_ok();
        if !installed && is_root {
            echo(
                &format!("  ⚠ {program} not found - install for web serving"),
                "yellow",
            );
        }
    }

    if !missing_deps.is_empty() {
        echo(
            &format!("  ⚠ Missing dependencies: {}", missing_deps.join(", ")),
            "yellow",
        );
        echo(
            &format!("  Install with: apt install {}", missing_deps.join(" ")),
            "yellow",
        );
    } else {
        echo("  ✓ All required dependencies found", "green");
    }
    echo("", "");
}
