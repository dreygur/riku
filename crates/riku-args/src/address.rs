//! Validated server name and ssh address types.
//!
//! Security model: `SshAddress` is later passed to the system `ssh` as an
//! argument, so it is the trust boundary. Construction accepts exactly one `@`.
//! The user and a hostname must match `[A-Za-z0-9._-]+` starting with an
//! alphanumeric, so neither can be read by ssh as an option. The host may
//! instead be one bracketed IPv6 literal, `[` + `[0-9A-Fa-f:.]+` + `]`, with
//! nothing after it. Whitespace, shell metacharacters, quotes, and path
//! separators are all outside the grammar. Values read from the TOML file go
//! through the same validation as values typed on the command line.

use anyhow::{anyhow, Result};

const MAX_NAME_LEN: usize = 32;

fn is_name_char(c: char) -> bool {
    c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-'
}

fn is_label_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')
}

/// `[A-Za-z0-9._-]+` starting with an alphanumeric, so ssh never sees an option.
fn is_label(s: &str) -> bool {
    s.chars().next().is_some_and(|c| c.is_ascii_alphanumeric()) && s.chars().all(is_label_char)
}

fn is_ipv6_literal(s: &str) -> bool {
    match s.strip_prefix('[').and_then(|rest| rest.strip_suffix(']')) {
        Some(inner) => {
            !inner.is_empty()
                && inner
                    .chars()
                    .all(|c| c.is_ascii_hexdigit() || matches!(c, ':' | '.'))
        }
        None => false,
    }
}

/// Name a server is known by locally, such as `edge`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerName(String);

impl TryFrom<&str> for ServerName {
    type Error = anyhow::Error;

    fn try_from(raw: &str) -> Result<Self> {
        let starts_alphanumeric = raw
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
        if !starts_alphanumeric || raw.len() > MAX_NAME_LEN || !raw.chars().all(is_name_char) {
            return Err(anyhow!(
                "invalid server name {raw:?}: use 1 to {MAX_NAME_LEN} characters of a-z, 0-9, '_' or '-', starting with a letter or digit"
            ));
        }
        Ok(ServerName(raw.to_string()))
    }
}

impl ServerName {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A `user@host` ssh target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SshAddress(String);

impl TryFrom<&str> for SshAddress {
    type Error = anyhow::Error;

    fn try_from(raw: &str) -> Result<Self> {
        let well_formed = raw.split_once('@').is_some_and(|(user, host)| {
            is_label(user) && (is_label(host) || is_ipv6_literal(host))
        });
        if !well_formed {
            return Err(anyhow!(
                "invalid ssh address {raw:?}: expected user@host with letters, digits and . _ - starting with a letter or digit (host may be a bracketed IPv6 literal)"
            ));
        }
        Ok(SshAddress(raw.to_string()))
    }
}

impl SshAddress {
    /// The login name before the `@`.
    pub fn user(&self) -> &str {
        self.0.split_once('@').map_or("", |(user, _)| user)
    }

    /// The address as typed, brackets and all. Use it for display and storage.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The argument to give `ssh`. ssh does not read brackets in a plain
    /// destination, so a bracketed IPv6 literal is passed without them.
    pub fn destination(&self) -> String {
        match self.0.split_once('@') {
            Some((user, host)) => {
                let bare = host
                    .strip_prefix('[')
                    .and_then(|rest| rest.strip_suffix(']'))
                    .unwrap_or(host);
                format!("{user}@{bare}")
            }
            None => self.0.clone(),
        }
    }
}

#[cfg(test)]
#[path = "address_tests.rs"]
mod tests;
