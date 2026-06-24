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

    // Validate NODE_VERSION if present
    if let Some(version) = env.get("NODE_VERSION") {
        if let Err(e) = validate_node_version(version) {
            warnings.push(e);
        }
    }

    check_nginx_cache_warnings(env, &mut warnings);

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
    if let Some(size_str) = env.get("NGINX_CACHE_SIZE") {
        if let Ok(size) = size_str.parse::<u32>() {
            if !(1..=100).contains(&size) {
                warnings.push(format!(
                    "Invalid NGINX_CACHE_SIZE: {} - must be between 1 and 100 GB",
                    size
                ));
            }
        } else {
            warnings.push(format!(
                "Invalid NGINX_CACHE_SIZE: '{}' - must be a number between 1 and 100",
                size_str
            ));
        }
    }

    for (key, label) in &[
        ("NGINX_CACHE_TIME", "NGINX_CACHE_TIME"),
        ("NGINX_CACHE_EXPIRY", "NGINX_CACHE_EXPIRY"),
        ("NGINX_CACHE_REDIRECTS", "NGINX_CACHE_REDIRECTS"),
        ("NGINX_CACHE_ANY", "NGINX_CACHE_ANY"),
        ("NGINX_CACHE_CONTROL", "NGINX_CACHE_CONTROL"),
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
mod tests {
    use super::*;

    #[test]
    fn test_validate_env_vars_deprecated_warnings() {
        let mut env = HashMap::new();
        env.insert("UWSGI_PROCESSES".to_string(), "4".to_string());
        env.insert("UWSGI_MAX_REQUESTS".to_string(), "1000".to_string());

        let warnings = validate_env_vars(&env);
        assert!(warnings.iter().any(|w| w.contains("UWSGI_PROCESSES")));
        assert!(warnings.iter().any(|w| w.contains("UWSGI_MAX_REQUESTS")));
    }

    #[test]
    fn test_validate_env_vars_node_version_warning() {
        let mut env = HashMap::new();
        env.insert("NODE_VERSION".to_string(), "invalid".to_string());

        let warnings = validate_env_vars(&env);
        assert!(warnings.iter().any(|w| w.contains("NODE_VERSION")));
    }

    #[test]
    fn test_validate_env_vars_clean() {
        let mut env = HashMap::new();
        env.insert("NGINX_SERVER_NAME".to_string(), "example.com".to_string());
        env.insert("BIND_ADDRESS".to_string(), "127.0.0.1".to_string());

        let warnings = validate_env_vars(&env);
        assert!(warnings.is_empty());
    }
}
