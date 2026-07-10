//! Input sanitization for nginx configuration values.
//!
//! Validates and sanitizes environment variable values before they are
//! inserted into nginx config templates, preventing nginx directive injection.
//!
//! # Why a deny-list, not an allow-list
//!
//! An allow-list is the generally safer default, but it was deliberately not
//! adopted here: `NGINX_SERVER_NAME` alone has to accommodate space/comma
//! separated hostname lists, IPv6 literals (`[::1]`), wildcards (`*.example.com`),
//! and regex-anchored forms (`~^(?<sub>.+)\.example\.com$`) — enumerating every
//! legitimate character across those forms without access to real-world usage
//! risks silently rejecting valid values in a running deployment, which is worse
//! than the deny-list it would replace. Instead, every character structurally
//! required to terminate an nginx directive and start another — `;`, `{`, `}`,
//! and newlines — is blocked outright, which is sufficient on its own regardless
//! of what else is allowed: nginx directives cannot be split, block-nested, or
//! chained without one of those four. The remaining entries (backtick, `$`,
//! quotes, `#`) are defense in depth against nginx variable interpolation and
//! template contexts that might quote or comment differently than today's.

use std::collections::HashMap;

const NGINX_DANGEROUS_CHARS: &[char] = &[';', '{', '}', '\n', '\r', '`', '$', '\\', '"', '\'', '#'];

/// Sanitize a value destined for an nginx config template.
/// Rejects values containing characters that could inject nginx directives.
pub(super) fn sanitize_nginx_value(key: &str, value: &str) -> anyhow::Result<String> {
    if value.chars().any(|c| NGINX_DANGEROUS_CHARS.contains(&c)) {
        return Err(anyhow::anyhow!(
            "Rejecting unsafe nginx config value for '{}': contains dangerous characters",
            key,
        ));
    }
    Ok(value.to_string())
}

/// Sanitize all environment variables before inserting into nginx template context.
/// Returns a new HashMap with validated values. Logs warnings for rejected values.
pub(super) fn sanitize_env_for_nginx(env: &HashMap<String, String>) -> HashMap<String, String> {
    let mut sanitized = HashMap::new();
    for (key, value) in env {
        match sanitize_nginx_value(key, value) {
            Ok(clean) => {
                sanitized.insert(key.clone(), clean);
            }
            Err(e) => {
                tracing::warn!("{}", e);
            }
        }
    }
    sanitized
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_nginx_value_rejects_semicolons() {
        assert!(
            sanitize_nginx_value("NGINX_SERVER_NAME", "example.com; proxy_pass http://evil")
                .is_err()
        );
    }

    #[test]
    fn test_sanitize_nginx_value_rejects_braces() {
        assert!(sanitize_nginx_value("NGINX_SERVER_NAME", "example.com { evil }").is_err());
    }

    #[test]
    fn test_sanitize_nginx_value_rejects_newlines() {
        assert!(sanitize_nginx_value("NGINX_SERVER_NAME", "example.com\nproxy_pass evil").is_err());
    }

    #[test]
    fn test_sanitize_nginx_value_rejects_backticks() {
        assert!(sanitize_nginx_value("PORT", "`curl evil.com`").is_err());
    }

    #[test]
    fn test_sanitize_nginx_value_rejects_hash() {
        assert!(sanitize_nginx_value("NGINX_SERVER_NAME", "example.com # comment").is_err());
    }

    #[test]
    fn test_sanitize_nginx_value_allows_clean_values() {
        assert!(sanitize_nginx_value("NGINX_SERVER_NAME", "example.com").is_ok());
        assert!(sanitize_nginx_value("PORT", "8080").is_ok());
        assert!(sanitize_nginx_value("BIND_ADDRESS", "127.0.0.1").is_ok());
        assert!(sanitize_nginx_value("NGINX_IPV6_ADDRESS", "[::]").is_ok());
    }

    #[test]
    fn test_sanitize_env_for_nginx_filters_dangerous() {
        let mut env = HashMap::new();
        env.insert("GOOD_KEY".to_string(), "clean-value".to_string());
        env.insert("BAD_KEY".to_string(), "value; inject".to_string());

        let sanitized = sanitize_env_for_nginx(&env);
        assert!(sanitized.contains_key("GOOD_KEY"));
        assert!(!sanitized.contains_key("BAD_KEY"));
    }
}
