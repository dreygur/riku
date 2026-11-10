//! Env keys riku renamed to the `RIKU_` prefix, and old keys whose feature
//! is gone.
//!
//! An app ENV that still sets an old name is not an error, but riku no longer
//! reads it, so deploy and `riku doctor` report each one: with its new name
//! when riku reads that, or as one to remove when the new name would only be
//! refused (the core route dropped it) or ignored. Only exact key matches
//! count: `NGINX_SERVER_NAME_EXTRA` is an app value.

use std::collections::HashMap;

/// What replaces an old key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Replacement {
    /// The key riku reads now.
    Key(&'static str),
    /// Nothing: the feature is gone, so the line should be removed.
    Removed,
}

use Replacement::{Key, Removed};

impl Replacement {
    /// What to tell the user about `old`.
    pub fn hint(self, old: &str) -> String {
        match self {
            Key(new) => format!("{old} is no longer read, use {new}"),
            Removed => format!("{old} is no longer supported; remove it from ENV"),
        }
    }
}

/// Every old key and what replaces it. Keys riku sets itself (such as the
/// generated nginx socket path) are left out, since no user ENV carries them.
pub const RENAMED_KEYS: &[(&str, Replacement)] = &[
    ("NGINX_SERVER_NAME", Key("RIKU_DOMAINS")),
    ("NGINX_HTTPS_ONLY", Key("RIKU_HTTPS_ONLY")),
    ("NGINX_INTERNAL_PORT", Key("RIKU_UPSTREAM_PORT")),
    ("BIND_ADDRESS", Key("RIKU_BIND_ADDRESS")),
    ("DISABLE_IPV6", Removed),
    ("ACME_EMAIL", Removed),
    ("ACME_ROOT_CA", Removed),
    ("ACME_ROOT", Key("RIKU_ACME_ROOT")),
    ("NODE_VERSION", Key("RIKU_NODE_VERSION")),
    ("PYTHON_VERSION", Key("RIKU_PYTHON_VERSION")),
    ("PYTHON_PACKAGE_MANAGER", Removed),
    ("RUNTIME", Key("RIKU_RUNTIME")),
    ("GHCR_USERNAME", Key("RIKU_GHCR_USERNAME")),
    ("GHCR_TOKEN", Key("RIKU_GHCR_TOKEN")),
    (
        "NGINX_ALLOW_GIT_FOLDERS",
        Key("RIKU_NGINX_ALLOW_GIT_FOLDERS"),
    ),
    ("NGINX_CACHE_ANY", Removed),
    ("NGINX_CACHE_CONTROL", Removed),
    ("NGINX_CACHE_EXPIRY", Removed),
    ("NGINX_CACHE_PATH", Removed),
    ("NGINX_CACHE_PREFIXES", Removed),
    ("NGINX_CACHE_REDIRECTS", Removed),
    ("NGINX_CACHE_SIZE", Removed),
    ("NGINX_CACHE_TIME", Removed),
    ("NGINX_CATCH_ALL", Removed),
    ("NGINX_CLOUDFLARE_ACL", Removed),
    ("NGINX_CSP", Removed),
    ("NGINX_DOCUMENT_ROOT", Key("RIKU_NGINX_DOCUMENT_ROOT")),
    ("NGINX_INCLUDE_FILE", Removed),
    ("NGINX_IPV4_ADDRESS", Removed),
    ("NGINX_IPV6_ADDRESS", Removed),
    ("NGINX_STATIC", Key("RIKU_NGINX_STATIC")),
    ("NGINX_STATIC_PATHS", Key("RIKU_NGINX_STATIC_PATHS")),
];

/// The old keys set in `env` and what replaces each, in table order.
pub fn legacy_keys(env: &HashMap<String, String>) -> Vec<(&'static str, Replacement)> {
    RENAMED_KEYS
        .iter()
        .filter(|(old, _)| env.contains_key(*old))
        .copied()
        .collect()
}

#[cfg(test)]
#[path = "legacy_env_tests.rs"]
mod tests;
