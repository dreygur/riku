//! Facts about the host that root init records in `/var/lib/riku/host.toml`.
//!
//! ```toml
//! init = "systemd"
//! ```
//!
//! Security model: the file is written only by root into a root owned
//! directory and may be read by anyone. A missing file means init has not
//! run on this host, which is different from any default, so [`read_host`]
//! returns `None` for it. A file that exists but holds an unknown key, an
//! unknown init system or anything that is not a TOML table is refused, so
//! a damaged file is never read as a guess.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use anyhow::{anyhow, Result};

/// File name of the host facts, directly under [`crate::VAR_ROOT`].
pub const HOST_FILE_NAME: &str = "host.toml";

/// The init system that starts services on the host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitSystem {
    Systemd,
    OpenRc,
    /// Anything else, including a host where neither signal was found.
    Other,
}

impl InitSystem {
    /// The value written to and read from `host.toml`.
    pub fn as_str(self) -> &'static str {
        match self {
            InitSystem::Systemd => "systemd",
            InitSystem::OpenRc => "openrc",
            InitSystem::Other => "other",
        }
    }

    fn parse(text: &str) -> Option<InitSystem> {
        match text {
            "systemd" => Some(InitSystem::Systemd),
            "openrc" => Some(InitSystem::OpenRc),
            "other" => Some(InitSystem::Other),
            _ => None,
        }
    }
}

/// Everything in `host.toml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostFacts {
    pub init: InitSystem,
}

impl HostFacts {
    /// The text root init writes to `host.toml`.
    pub fn to_toml(&self) -> String {
        format!("init = \"{}\"\n", self.init.as_str())
    }

    fn from_table(table: &toml::Table, file: &Path) -> Result<HostFacts> {
        if let Some(key) = table.keys().find(|key| key.as_str() != "init") {
            return Err(anyhow!("{}: unknown key `{key}`", file.display()));
        }
        let init = table
            .get("init")
            .and_then(toml::Value::as_str)
            .and_then(InitSystem::parse);
        match init {
            Some(init) => Ok(HostFacts { init }),
            None => Err(anyhow!(
                "{}: init must be \"systemd\", \"openrc\" or \"other\"",
                file.display()
            )),
        }
    }
}

/// Read the host facts at `path`. `Ok(None)` means the file does not exist.
pub fn read_host(path: &Path) -> Result<Option<HostFacts>> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(anyhow!("cannot read {}: {e}", path.display())),
    };
    let table = match text.parse::<toml::Table>() {
        Ok(table) => table,
        Err(e) => return Err(anyhow!("{}: {e}", path.display())),
    };
    HostFacts::from_table(&table, path).map(Some)
}

#[cfg(test)]
#[path = "host_file_tests.rs"]
mod tests;
