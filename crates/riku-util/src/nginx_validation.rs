//! Nginx-specific configuration validation.
//!
//! # Security Model
//!
//! Cache configuration values come from user-supplied environment variables.
//! All numeric fields are parsed and range-checked before use in templates
//! to prevent injection of unexpected values into nginx config files.

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
                "Warning: {} is deprecated - Riku uses a custom supervisor instead of uWSGI. \
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

    check_nginx_cache_warnings(env, &mut warnings);

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

/// Append warnings for any invalid nginx cache env vars.
fn check_nginx_cache_warnings(env: &HashMap<String, String>, warnings: &mut Vec<String>) {
    if let Some(size_str) = env.get("RIKU_NGINX_CACHE_SIZE") {
        if let Ok(size) = size_str.parse::<u32>() {
            if !(1..=100).contains(&size) {
                warnings.push(format!(
                    "Invalid RIKU_NGINX_CACHE_SIZE: {} - must be between 1 and 100 GB",
                    size
                ));
            }
        } else {
            warnings.push(format!(
                "Invalid RIKU_NGINX_CACHE_SIZE: '{}' - must be a number between 1 and 100",
                size_str
            ));
        }
    }

    for (key, label) in &[
        ("RIKU_NGINX_CACHE_TIME", "RIKU_NGINX_CACHE_TIME"),
        ("RIKU_NGINX_CACHE_EXPIRY", "RIKU_NGINX_CACHE_EXPIRY"),
        ("RIKU_NGINX_CACHE_REDIRECTS", "RIKU_NGINX_CACHE_REDIRECTS"),
        ("RIKU_NGINX_CACHE_ANY", "RIKU_NGINX_CACHE_ANY"),
        ("RIKU_NGINX_CACHE_CONTROL", "RIKU_NGINX_CACHE_CONTROL"),
    ] {
        if let Some(val) = env.get(*key) {
            if val.parse::<u32>().is_err() {
                warnings.push(format!(
                    "Invalid {}: '{}' - must be a positive integer (seconds)",
                    label, val
                ));
            }
        }
    }
}

#[cfg(test)]
#[path = "nginx_validation_tests.rs"]
mod tests;
