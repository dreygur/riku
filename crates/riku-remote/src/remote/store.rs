//! Reads and writes the server list TOML file.
//!
//! The file lives at `${XDG_CONFIG_HOME:-$HOME/.config}/riku/servers.toml`
//! (not `dirs::config_dir()`, which is `~/Library/Application Support` on
//! macOS). It holds ssh targets, so the directory is created 700 and the file
//! 600, written to a temp file in the same directory and renamed into place.
//! Every entry is revalidated on load through the same `TryFrom` impls as
//! command line input, so a hand edited bad entry is rejected with the file
//! path and entry name.

use super::types::{Server, ServerList, ServerName, SshAddress};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Default)]
struct RawFile {
    default: Option<String>,
    #[serde(default)]
    server: Vec<RawServer>,
}

#[derive(Serialize, Deserialize)]
struct RawServer {
    name: String,
    address: String,
    admin: Option<String>,
}

/// Location of the server list for the current user.
pub fn servers_path() -> Result<PathBuf> {
    let home = match dirs::home_dir() {
        Some(home) => home,
        None => return Err(anyhow!("cannot determine the home directory")),
    };
    let xdg = std::env::var("XDG_CONFIG_HOME").ok();
    Ok(servers_path_from(xdg.as_deref(), &home.to_string_lossy()))
}

/// Resolves the path from explicit values. An empty or relative `xdg` counts as unset, per the XDG spec.
pub fn servers_path_from(xdg: Option<&str>, home: &str) -> PathBuf {
    let base = match xdg {
        Some(dir) if Path::new(dir).is_absolute() => PathBuf::from(dir),
        _ => Path::new(home).join(".config"),
    };
    base.join("riku").join("servers.toml")
}

fn parse_server(path: &Path, raw: RawServer) -> Result<Server> {
    let fail = |what: &str, e: anyhow::Error| {
        anyhow!("{}: server '{}': {what}: {e}", path.display(), raw.name)
    };
    let name = match ServerName::try_from(raw.name.as_str()) {
        Ok(name) => name,
        Err(e) => return Err(fail("name", e)),
    };
    let address = match SshAddress::try_from(raw.address.as_str()) {
        Ok(address) => address,
        Err(e) => return Err(fail("address", e)),
    };
    let admin = match raw.admin.as_deref().map(SshAddress::try_from) {
        None => None,
        Some(Ok(admin)) => Some(admin),
        Some(Err(e)) => return Err(fail("admin", e)),
    };
    Ok(Server {
        name,
        address,
        admin,
    })
}

#[allow(clippy::question_mark)]
fn into_list(path: &Path, raw: RawFile) -> Result<ServerList> {
    let mut list = ServerList::default();
    for entry in raw.server {
        let server = match parse_server(path, entry) {
            Ok(server) => server,
            Err(e) => return Err(e),
        };
        if let Err(e) = list.add(server) {
            return Err(anyhow!("{}: {e}", path.display()));
        }
    }
    if let Some(default) = raw.default {
        let named = ServerName::try_from(default.as_str()).and_then(|n| list.set_default(&n));
        if let Err(e) = named {
            return Err(anyhow!("{}: default server: {e}", path.display()));
        }
    }
    Ok(list)
}

/// Loads the list. A missing file is an empty list.
pub fn load(path: &Path) -> Result<ServerList> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(ServerList::default()),
        Err(e) => return Err(anyhow!("failed to read {}: {e}", path.display())),
    };
    let raw: RawFile = match toml::from_str(&text) {
        Ok(raw) => raw,
        Err(e) => return Err(anyhow!("failed to parse {}: {e}", path.display())),
    };
    into_list(path, raw)
}

fn to_raw(list: &ServerList) -> RawFile {
    RawFile {
        default: list.default_server().map(|s| s.name.as_str().to_string()),
        server: list
            .iter()
            .map(|s| RawServer {
                name: s.name.as_str().to_string(),
                address: s.address.as_str().to_string(),
                admin: s.admin.as_ref().map(|a| a.as_str().to_string()),
            })
            .collect(),
    }
}

#[allow(clippy::question_mark)]
fn write_private(dir: &Path, path: &Path, text: &str) -> std::io::Result<()> {
    let mut temp = match tempfile::NamedTempFile::new_in(dir) {
        Ok(temp) => temp,
        Err(e) => return Err(e),
    };
    if let Err(e) = temp
        .as_file()
        .set_permissions(std::fs::Permissions::from_mode(0o600))
    {
        return Err(e);
    }
    if let Err(e) = temp.write_all(text.as_bytes()) {
        return Err(e);
    }
    match temp.persist(path) {
        Ok(_) => Ok(()),
        Err(e) => Err(e.error),
    }
}

/// Saves the list atomically with mode 600, creating the directory with mode 700.
pub fn save(path: &Path, list: &ServerList) -> Result<()> {
    let dir = match path.parent() {
        Some(dir) => dir,
        None => return Err(anyhow!("{} has no parent directory", path.display())),
    };
    if let Err(e) = std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
    {
        return Err(anyhow!("failed to create {}: {e}", dir.display()));
    }
    let text = match toml::to_string_pretty(&to_raw(list)) {
        Ok(text) => text,
        Err(e) => return Err(anyhow!("failed to serialize server list: {e}")),
    };
    if let Err(e) = write_private(dir, path, &text) {
        return Err(anyhow!("failed to write {}: {e}", path.display()));
    }
    Ok(())
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
