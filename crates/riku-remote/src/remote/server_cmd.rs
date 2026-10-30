//! `riku server add|list|remove|default`: edits the local server list.
//!
//! Security model: names and addresses typed here are validated by
//! `ServerName` and `SshAddress` before anything is stored, so the file only
//! ever holds values that are safe to hand to `ssh`.

use std::path::Path;

use anyhow::{anyhow, Result};

use super::store;
use super::types::{Server, ServerList, ServerName, SshAddress};
use crate::cmds::ServerCmd;
use crate::util::display;

const DEFAULT_MARK: &str = "*";

#[allow(clippy::question_mark)]
fn build_server(name: &str, address: &str, admin: Option<&str>) -> Result<Server> {
    let name = match ServerName::try_from(name) {
        Ok(name) => name,
        Err(e) => return Err(e),
    };
    let address = match SshAddress::try_from(address) {
        Ok(address) => address,
        Err(e) => return Err(e),
    };
    let admin = match admin.map(SshAddress::try_from) {
        None => None,
        Some(Ok(admin)) => Some(admin),
        Some(Err(e)) => return Err(e),
    };
    Ok(Server {
        name,
        address,
        admin,
    })
}

fn print_servers(list: &ServerList) {
    let default = list.default_server().map(|s| s.name.as_str());
    let rows: Vec<Vec<String>> = list
        .iter()
        .map(|s| {
            let mark = if Some(s.name.as_str()) == default {
                DEFAULT_MARK
            } else {
                ""
            };
            vec![
                format!("{}{mark}", s.name.as_str()),
                s.address.as_str().to_string(),
                s.admin.as_ref().map_or("", |a| a.as_str()).to_string(),
            ]
        })
        .collect();
    display::print_table(&["NAME", "ADDRESS", "ADMIN"], &rows, 2);
}

#[allow(clippy::question_mark)]
fn mutate(path: &Path, change: impl FnOnce(&mut ServerList) -> Result<()>) -> Result<()> {
    let mut list = match store::load(path) {
        Ok(list) => list,
        Err(e) => return Err(e),
    };
    if let Err(e) = change(&mut list) {
        return Err(e);
    }
    store::save(path, &list)
}

/// Runs a server subcommand against the list stored at `path`.
#[allow(clippy::question_mark)]
pub fn run_at(path: &Path, cmd: ServerCmd) -> Result<()> {
    match cmd {
        ServerCmd::Add {
            name,
            address,
            admin,
        } => {
            let server = match build_server(&name, &address, admin.as_deref()) {
                Ok(server) => server,
                Err(e) => return Err(e),
            };
            mutate(path, |list| list.add(server))
        }
        ServerCmd::List => match store::load(path) {
            Ok(list) => {
                print_servers(&list);
                Ok(())
            }
            Err(e) => Err(e),
        },
        ServerCmd::Remove { name } => {
            let name = match ServerName::try_from(name.as_str()) {
                Ok(name) => name,
                Err(e) => return Err(e),
            };
            mutate(path, |list| list.remove(&name))
        }
        ServerCmd::Default { name } => {
            let name = match ServerName::try_from(name.as_str()) {
                Ok(name) => name,
                Err(e) => return Err(e),
            };
            mutate(path, |list| list.set_default(&name))
        }
    }
}

/// Runs a server subcommand against the current user's server list.
pub fn run(cmd: ServerCmd) -> Result<()> {
    match store::servers_path() {
        Ok(path) => run_at(&path, cmd),
        Err(e) => Err(anyhow!("cannot locate the server list: {e}")),
    }
}
