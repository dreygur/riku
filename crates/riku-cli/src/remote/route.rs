//! Decides where a parsed command runs and forwards it when that is a server.
//!
//! Security model: the server list and `git remote -v` are only read for
//! forwarded commands started by a person on this machine. When riku runs as
//! the ssh forced command (`SSH_ORIGINAL_COMMAND` set) or the command is
//! local or server only, neither is touched, so server side invocations,
//! git push hooks included, behave exactly as before forwarding existed.

use super::git_remote::read_remotes;
use super::target::{resolve, Selection, Target};
use super::{forward, store, Server};
use crate::placement::{placement_of, Placement};
use anyhow::{anyhow, Result};

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

/// Target for the top-level command `command`.
///
/// Forwarded commands go through the full precedence. A server only command
/// given `-s` is refused instead of silently running here. Everything else
/// runs locally.
pub fn target_for(command: &str, sel: Selection) -> Result<Target> {
    if sel.local || sel.via_ssh {
        return Ok(Target::Local);
    }
    match placement_of(command) {
        Some(Placement::Forwarded) => resolve_from_disk(sel),
        Some(Placement::Server) if sel.flag.is_some() => Err(anyhow!(
            "'{command}' runs on the server itself; use --local to run it here"
        )),
        _ => Ok(Target::Local),
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

/// Prints the target line, runs the command on `server`, and returns its
/// exit code. `argv` is the full process argv, program name included.
#[allow(clippy::question_mark)]
pub fn forward(argv: &[String], server: &Server, app_hint: Option<&str>) -> Result<i32> {
    let cli = crate::help::command(crate::help::FORWARD_HEADING);
    let consumed = match forward::server_arg_positions(&cli, argv) {
        Ok(consumed) => consumed,
        Err(e) => return Err(e),
    };
    let args = forward::strip_target_args(argv, &consumed);
    eprintln!("[*] {} ({})", server.name.as_str(), server.address.as_str());
    forward::run(server, args, app_hint)
}
