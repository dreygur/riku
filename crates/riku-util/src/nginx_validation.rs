//! Nginx-specific configuration validation.
//!
//! # Security Model
//!
//! The values come from user-supplied environment variables, so this only
//! warns; the route gate decides what reaches an nginx config file.

use std::collections::HashMap;

use super::display::echo;
use super::process_util::validate_node_version;

/// Validate environment variables and return warnings/errors.
pub fn validate_env_vars(env: &HashMap<String, String>) -> Vec<String> {
    let mut warnings = Vec::new();

    // Check for deprecated uWSGI variables
    let deprecated_vars = [
        "UWSGI_MAX_REQUESTS",
        "UWSGI_PROCESSES",
        "UWSGI_LISTEN",
        "UWSGI_ENABLE_THREADS",
        "UWSGI_LOG_MAXSIZE",
        "UWSGI_IDLE",
        "UWSGI_GEVENT",
        "UWSGI_ASYNCIO",
        "UWSGI_INCLUDE_FILE",
    ];

    for var in &deprecated_vars {
        if env.contains_key(*var) {
            warnings.push(format!(
                "{} is deprecated - Riku uses a custom supervisor instead of uWSGI. \
                 Use RIKU_* variables or the SCALING file instead.",
                var
            ));
        }
    }

    // Validate RIKU_NODE_VERSION if present
    if let Some(version) = env.get("RIKU_NODE_VERSION") {
        if let Err(e) = validate_node_version(version) {
            warnings.push(e);
        }
    }

    if let Some(warning) = super::flag::https_only_warning(env) {
        warnings.push(warning);
    }

    warnings
}

/// Print environment variable validation warnings.
pub fn print_env_warnings(warnings: &[String]) {
    for warning in warnings {
        echo(warning, "yellow");
    }
}

#[cfg(test)]
#[path = "nginx_validation_tests.rs"]
mod tests;
