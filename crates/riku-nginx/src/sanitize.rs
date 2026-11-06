//! Input sanitization for values written into nginx config.
//!
//! One deny-list, [`breaks_directive`], decides which characters can end an
//! nginx directive, open or close a block, or start a comment, an
//! interpolation or a quoted string. The router helper's route gate uses the
//! same function, so the gate and the renderer cannot disagree.

/// Whether `c` could end an nginx directive or start another inside a value
/// written unquoted into a config file. Spaces are allowed here because a
/// domain list is space separated; callers that forbid them check for them.
pub fn breaks_directive(c: char) -> bool {
    c.is_control() || matches!(c, ';' | '{' | '}' | '#' | '"' | '\'' | '$' | '`' | '\\')
}

/// Reject a value destined for an nginx config template when it holds a
/// character that could inject a directive.
pub(super) fn sanitize_nginx_value(key: &str, value: &str) -> anyhow::Result<String> {
    if value.chars().any(breaks_directive) {
        return Err(anyhow::anyhow!(
            "Rejecting unsafe nginx config value for '{}': contains dangerous characters",
            key,
        ));
    }
    Ok(value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_values_that_end_or_open_a_directive() {
        for value in [
            "example.com; proxy_pass http://evil",
            "example.com { evil }",
            "example.com\nproxy_pass evil",
            "`curl evil.com`",
            "example.com # comment",
        ] {
            assert!(
                sanitize_nginx_value("RIKU_DOMAINS", value).is_err(),
                "{value:?} was accepted"
            );
        }
    }

    #[test]
    fn allows_clean_values() {
        assert!(sanitize_nginx_value("RIKU_DOMAINS", "a.example.com b.example.com").is_ok());
        assert!(sanitize_nginx_value("PORT", "8080").is_ok());
    }
}
