//! Pre-flight checks for `riku init`: root warning and missing tools.

use std::process::Command;

use crate::config::router::Router;
use crate::util::display;

use super::router_choice::installed_routers;

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

/// The tools the prerequisites step found: git, then each installed router,
/// or `no router`.
fn found_tools(git: bool, routers: &[Router]) -> String {
    let mut found: Vec<&str> = Vec::new();
    if git {
        found.push("git");
    }
    if routers.is_empty() {
        found.push("no router");
    }
    found.extend(routers.iter().map(Router::name));
    found.join(", ")
}

/// Report git and the routers installed on this host, or missing git. Which
/// router to use, and what to do without one, is settled when init chooses
/// it (see [`super::router_choice`]).
pub fn check_prerequisites() {
    let routers = installed_routers();
    if Command::new("git").arg("--version").output().is_ok() {
        display::step_done("Prerequisites", &found_tools(true, &routers));
        return;
    }
    display::warn("Missing git");
    display::detail("Install it with: apt install git");
}

#[cfg(test)]
#[path = "prerequisites_tests.rs"]
mod tests;
