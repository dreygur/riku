//! Validated server list types.
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
    pub fn user(&self) -> &str {
        self.0.split_once('@').map_or("", |(user, _)| user)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One remote riku server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Server {
    pub name: ServerName,
    /// Address of the restricted riku user that forwarded commands run as.
    pub address: SshAddress,
    /// Address of an admin account, for commands that need a full shell.
    pub admin: Option<SshAddress>,
}

/// The configured servers and which one is the default.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ServerList {
    servers: Vec<Server>,
    default: Option<ServerName>,
}

impl ServerList {
    /// The server registered under `name`, if any.
    pub fn get(&self, name: &ServerName) -> Option<&Server> {
        self.servers.iter().find(|s| &s.name == name)
    }

    /// The default server, or `None` when none is set.
    pub fn default_server(&self) -> Option<&Server> {
        self.default.as_ref().and_then(|name| self.get(name))
    }

    /// Adds a server. A name that is already present is an error.
    pub fn add(&mut self, server: Server) -> Result<()> {
        if self.get(&server.name).is_some() {
            return Err(anyhow!("server '{}' already exists", server.name.as_str()));
        }
        self.servers.push(server);
        Ok(())
    }

    /// Removes a server, clearing the default if it pointed at this one.
    pub fn remove(&mut self, name: &ServerName) -> Result<()> {
        let before = self.servers.len();
        self.servers.retain(|s| &s.name != name);
        if self.servers.len() == before {
            return Err(anyhow!("no server named '{}'", name.as_str()));
        }
        if self.default.as_ref() == Some(name) {
            self.default = None;
        }
        Ok(())
    }

    /// Makes `name` the default server. It must already be registered.
    pub fn set_default(&mut self, name: &ServerName) -> Result<()> {
        if self.get(name).is_none() {
            return Err(anyhow!("no server named '{}'", name.as_str()));
        }
        self.default = Some(name.clone());
        Ok(())
    }

    /// Servers in the order they were added.
    pub fn iter(&self) -> impl Iterator<Item = &Server> {
        self.servers.iter()
    }
}

#[cfg(test)]
#[path = "types_tests.rs"]
mod tests;
