//! Pre-flight checks for `riku init`: root warning and missing tools.

use std::process::Command;

use crate::config::router::Router;
use crate::util::echo;

/// The router's display name, `None` for a plugin router.
fn router_label(router: &Router) -> Option<&'static str> {
    match router {
        Router::Nginx => Some("Nginx"),
        Router::Caddy => Some("Caddy"),
        Router::Plugin(_) => None,
    }
}

/// Tell a non-root caller what a root init would also set up.
pub fn warn_not_root(router: &Router) {
    echo("⚠ Warning: Not running as root", "yellow");
    echo("  Some features require root privileges:", "yellow");
    echo("    - System-wide systemd service", "yellow");
    if let Some(label) = router_label(router) {
        echo(&format!("    - {label} configuration"), "yellow");
    }
    echo("  Run 'sudo riku init' for full installation.", "yellow");
    echo("", "");
}

/// Report missing git. The router binary is checked earlier, when init
/// chooses the router (see [`super::router_choice`]).
pub fn check_prerequisites() {
    echo("Checking prerequisites...", "");
    let mut missing_deps = Vec::new();

    if Command::new("git").arg("--version").output().is_err() {
        missing_deps.push("git");
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
