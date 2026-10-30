//! In-memory server list built from the validated address types.

use anyhow::{anyhow, Result};
pub use riku_args::address::{ServerName, SshAddress};

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
