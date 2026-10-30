//! Decides where a parsed command runs and forwards it when that is a server.
//!
//! Security model: the server list and `git remote -v` are only read for
//! forwarded commands and `dashboard` started by a person on this machine.
//! When riku runs as the ssh forced command (`SSH_ORIGINAL_COMMAND` set) or
//! the command is local or any other server only command, neither is
//! touched, so server side invocations, git push hooks included, behave
//! exactly as before forwarding existed.

use super::git_remote::read_remotes;
use super::target::{resolve, Selection, Source, Target};
use super::{forward, store, Server};
use crate::placement::{placement_of, Placement};
use anyhow::{anyhow, Result};

/// The one server only command that can target a server.
const DASHBOARD: &str = "dashboard";

#[allow(clippy::question_mark)]
fn resolve_from_disk(sel: Selection) -> Result<Target> {
    let path = match store::servers_path() {
        Ok(path) => path,
        Err(e) => return Err(e),
    };
    let list = match store::load(&path) {
        Ok(list) => list,
        Err(e) => return Err(e),
    };
    resolve(sel, &list, &read_remotes())
}

/// How a top-level command picks its target.
#[derive(Debug, PartialEq, Eq)]
enum Routing {
    /// Apply the full precedence: flag, git remote, default, local.
    Resolve,
    /// A server only command was given `-s`.
    Refuse,
    Local,
}

/// `plugins` subcommands that create keys or sign files for the author, so
/// they must never run on a server.
const PLUGIN_AUTHORING: [&str; 3] = ["keygen", "sign", "scaffold"];

/// True for the `plugins` subcommands that only make sense on the author's machine.
pub fn is_plugin_authoring(command: &str, subcommand: Option<&str>) -> bool {
    command == "plugins" && subcommand.is_some_and(|sub| PLUGIN_AUTHORING.contains(&sub))
}

/// Forwarded commands and `dashboard` resolve a target; a remote dashboard
/// is opened locally rather than forwarded. Any other server only command
/// given `-s` is refused instead of silently running here.
fn routing(command: &str, subcommand: Option<&str>, flag_given: bool) -> Routing {
    match placement_of(command) {
        Some(Placement::Forwarded) if is_plugin_authoring(command, subcommand) => Routing::Local,
        Some(Placement::Forwarded) => Routing::Resolve,
        Some(Placement::Server) if command == DASHBOARD => Routing::Resolve,
        Some(Placement::Server) if flag_given => Routing::Refuse,
        _ => Routing::Local,
    }
}

/// Target for the top-level command `command`, whose own subcommand (if
/// any) is `subcommand`.
pub fn target_for(command: &str, subcommand: Option<&str>, sel: Selection) -> Result<Target> {
    if sel.local || sel.via_ssh {
        return Ok(Target::Local);
    }
    match routing(command, subcommand, sel.flag.is_some()) {
        Routing::Resolve => resolve_from_disk(sel),
        Routing::Refuse => Err(anyhow!(
            "'{command}' runs on the server itself; use --local to run it here"
        )),
        Routing::Local => Ok(Target::Local),
    }
}

/// Server a forwarded command without `-s` would reach, for the help
/// heading. Any problem reading the list or remotes means none.
pub fn implied_server(via_ssh: bool) -> Option<Server> {
    if via_ssh {
        return None;
    }
    let sel = Selection {
        flag: None,
        local: false,
        via_ssh,
    };
    match resolve_from_disk(sel) {
        Ok(Target::Remote { server, .. }) => Some(server),
        _ => None,
    }
}

fn target_line(server: &Server, source: Source, app_hint: Option<&str>) -> String {
    let base = format!("[*] {} ({})", server.name.as_str(), server.address.as_str());
    match (source, app_hint) {
        (Source::GitRemote, Some(app)) => format!("{base} app {app} from git remote"),
        _ => base,
    }
}

/// Prints the target line, runs the command on `server`, and returns its
/// exit code. `argv` is the full process argv, program name included.
/// `source` is why `server` was chosen; `app_hint` is the app sent along.
#[allow(clippy::question_mark)]
pub fn forward(
    argv: &[String],
    server: &Server,
    source: Source,
    app_hint: Option<&str>,
) -> Result<i32> {
    let cli = crate::help::command(crate::help::FORWARD_HEADING);
    let consumed = match forward::server_arg_positions(&cli, argv) {
        Ok(consumed) => consumed,
        Err(e) => return Err(e),
    };
    let args = forward::strip_target_args(argv, &consumed);
    eprintln!("{}", target_line(server, source, app_hint));
    forward::run(server, args, app_hint)
}

#[cfg(test)]
#[path = "route_tests.rs"]
mod tests;
