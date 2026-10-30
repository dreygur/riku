//! Picks the server a forwarded command runs on.

use super::git_remote::RemoteRef;
use super::{Server, ServerList};
use anyhow::{anyhow, Result};

/// Why a server was selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Flag,
    GitRemote,
    Default,
}

/// Where a command runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    Local,
    Remote {
        server: Server,
        source: Source,
        /// App name from the matching git remote. Only set for `Source::GitRemote`.
        app_hint: Option<String>,
    },
}

/// What the command line says about the target.
#[derive(Debug, Clone, Copy)]
pub struct Selection<'a> {
    /// Value of `-s/--server`.
    pub flag: Option<&'a str>,
    /// `--local` was given.
    pub local: bool,
    /// Riku is running as the server side ssh forced command.
    pub via_ssh: bool,
}

/// The server names in `list` as a phrase for error messages.
pub fn registered_names(list: &ServerList) -> String {
    let names: Vec<&str> = list.iter().map(|s| s.name.as_str()).collect();
    if names.is_empty() {
        return "none are registered".to_string();
    }
    format!("registered: {}", names.join(", "))
}

fn by_flag(flag: &str, list: &ServerList) -> Result<Target> {
    match list.iter().find(|s| s.name.as_str() == flag) {
        Some(server) => Ok(Target::Remote {
            server: server.clone(),
            source: Source::Flag,
            app_hint: None,
        }),
        None => Err(anyhow!(
            "no server named '{flag}' ({})",
            registered_names(list)
        )),
    }
}

/// The server whose address exactly equals a remote's `user@host`. The app
/// hint is dropped when remotes on that server name different apps.
fn by_git_remote(list: &ServerList, remotes: &[RemoteRef]) -> Result<Option<Target>> {
    let matched: Vec<(&Server, &str)> = remotes
        .iter()
        .filter_map(|r| {
            list.iter()
                .find(|s| s.address.as_str() == r.address)
                .map(|s| (s, r.app.as_str()))
        })
        .collect();
    let (server, app) = match matched.first() {
        Some(&first) => first,
        None => return Ok(None),
    };
    if let Some((other, _)) = matched.iter().find(|(s, _)| s.name != server.name) {
        return Err(anyhow!(
            "this repo has git remotes on two servers, '{}' and '{}'; choose one with -s <name>",
            server.name.as_str(),
            other.name.as_str()
        ));
    }
    let same_app = matched.iter().all(|&(_, a)| a == app);
    Ok(Some(Target::Remote {
        server: server.clone(),
        source: Source::GitRemote,
        app_hint: same_app.then(|| app.to_string()),
    }))
}

/// Applies the precedence flag, git remote, default, local.
///
/// `--local` and a server side invocation always mean local, so riku running
/// as the forced command never forwards again.
pub fn resolve(sel: Selection, list: &ServerList, remotes: &[RemoteRef]) -> Result<Target> {
    if sel.local || sel.via_ssh {
        return Ok(Target::Local);
    }
    if let Some(flag) = sel.flag {
        return by_flag(flag, list);
    }
    match by_git_remote(list, remotes) {
        Ok(Some(target)) => return Ok(target),
        Ok(None) => {}
        Err(e) => return Err(e),
    }
    Ok(match list.default_server() {
        Some(server) => Target::Remote {
            server: server.clone(),
            source: Source::Default,
            app_hint: None,
        },
        None => Target::Local,
    })
}

#[cfg(test)]
#[path = "target_tests.rs"]
mod tests;
