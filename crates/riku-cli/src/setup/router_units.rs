//! How riku's own systemd service orders itself after the host router.
//!
//! Reloads go through the router helper (see [`super::helper_install`]), so
//! init writes no path unit or reload service for the router.

use crate::config::router::Router;

/// The unit riku's own service orders after: `nginx.service`,
/// `caddy.service`, or empty for a plugin router.
pub fn router_unit_after(router: &Router) -> &'static str {
    match router {
        Router::Nginx => "nginx.service",
        Router::Caddy => "caddy.service",
        Router::Plugin(_) => "",
    }
}

/// The `After=` and `Wants=` lines for a riku service unit.
pub fn unit_ordering(router: &Router) -> String {
    match router_unit_after(router) {
        "" => "After=network.target".to_string(),
        unit => format!("After=network.target {unit}\nWants={unit}"),
    }
}

#[cfg(test)]
#[path = "router_units_tests.rs"]
mod tests;
