//! Env keys riku renamed to the `RIKU_` prefix.
//!
//! An app ENV that still sets an old name is not an error, but riku no longer
//! reads it, so deploy and `riku doctor` report each one with its new name.
//! Only exact key matches count: `NGINX_SERVER_NAME_EXTRA` is an app value.

use std::collections::HashMap;

/// Every renamed key as `(old, new)`. Keys riku sets itself (such as the
/// generated nginx socket path) are left out, since no user ENV carries them.
pub const RENAMED_KEYS: &[(&str, &str)] = &[
    ("NGINX_SERVER_NAME", "RIKU_DOMAINS"),
    ("NGINX_HTTPS_ONLY", "RIKU_HTTPS_ONLY"),
    ("NGINX_INTERNAL_PORT", "RIKU_UPSTREAM_PORT"),
    ("BIND_ADDRESS", "RIKU_BIND_ADDRESS"),
    ("DISABLE_IPV6", "RIKU_DISABLE_IPV6"),
    ("ACME_EMAIL", "RIKU_ACME_EMAIL"),
    ("ACME_ROOT_CA", "RIKU_ACME_ROOT_CA"),
    ("ACME_ROOT", "RIKU_ACME_ROOT"),
    ("NODE_VERSION", "RIKU_NODE_VERSION"),
    ("PYTHON_VERSION", "RIKU_PYTHON_VERSION"),
    ("PYTHON_PACKAGE_MANAGER", "RIKU_PYTHON_PACKAGE_MANAGER"),
    ("RUNTIME", "RIKU_RUNTIME"),
    ("GHCR_USERNAME", "RIKU_GHCR_USERNAME"),
    ("GHCR_TOKEN", "RIKU_GHCR_TOKEN"),
    ("NGINX_ALLOW_GIT_FOLDERS", "RIKU_NGINX_ALLOW_GIT_FOLDERS"),
    ("NGINX_CACHE_ANY", "RIKU_NGINX_CACHE_ANY"),
    ("NGINX_CACHE_CONTROL", "RIKU_NGINX_CACHE_CONTROL"),
    ("NGINX_CACHE_EXPIRY", "RIKU_NGINX_CACHE_EXPIRY"),
    ("NGINX_CACHE_PATH", "RIKU_NGINX_CACHE_PATH"),
    ("NGINX_CACHE_PREFIXES", "RIKU_NGINX_CACHE_PREFIXES"),
    ("NGINX_CACHE_REDIRECTS", "RIKU_NGINX_CACHE_REDIRECTS"),
    ("NGINX_CACHE_SIZE", "RIKU_NGINX_CACHE_SIZE"),
    ("NGINX_CACHE_TIME", "RIKU_NGINX_CACHE_TIME"),
    ("NGINX_CATCH_ALL", "RIKU_NGINX_CATCH_ALL"),
    ("NGINX_CLOUDFLARE_ACL", "RIKU_NGINX_CLOUDFLARE_ACL"),
    ("NGINX_CSP", "RIKU_NGINX_CSP"),
    ("NGINX_DOCUMENT_ROOT", "RIKU_NGINX_DOCUMENT_ROOT"),
    ("NGINX_EXTERNAL_PORT", "RIKU_NGINX_EXTERNAL_PORT"),
    ("NGINX_INCLUDE_FILE", "RIKU_NGINX_INCLUDE_FILE"),
    ("NGINX_IPV4_ADDRESS", "RIKU_NGINX_IPV4_ADDRESS"),
    ("NGINX_IPV6_ADDRESS", "RIKU_NGINX_IPV6_ADDRESS"),
    ("NGINX_STATIC", "RIKU_NGINX_STATIC"),
    ("NGINX_STATIC_PATHS", "RIKU_NGINX_STATIC_PATHS"),
];

/// The `(old, new)` pairs whose old name is a key in `env`, in table order.
pub fn legacy_keys(env: &HashMap<String, String>) -> Vec<(&'static str, &'static str)> {
    RENAMED_KEYS
        .iter()
        .filter(|(old, _)| env.contains_key(*old))
        .copied()
        .collect()
}

#[cfg(test)]
#[path = "legacy_env_tests.rs"]
mod tests;
