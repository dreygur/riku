//! Pre-flight checks for `riku init`: root warning and missing tools.

use std::process::Command;

use crate::config::router::Router;
use crate::util::display;

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
    let extra = match router_label(router) {
        Some(label) => format!("the system service and the {label} configuration"),
        None => "the system service".to_string(),
    };
    display::warn("Not running as root");
    display::detail(&format!("sudo riku init also sets up {extra}"));
}

/// Report missing git. The router binary is checked when init chooses the
/// router (see [`super::router_choice`]).
pub fn check_prerequisites() {
    if Command::new("git").arg("--version").output().is_ok() {
        display::step_done("Prerequisites", "git");
        return;
    }
    display::warn("Missing git");
    display::detail("Install it with: apt install git");
}
