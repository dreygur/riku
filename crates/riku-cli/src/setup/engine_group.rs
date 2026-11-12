//! Root init's offer to let the deploy user reach docker's socket.
//!
//! Container apps need the engine. With docker that means the socket at
//! `/var/run/docker.sock`, and membership in the group that owns it is
//! root-equivalent on the host. So init only asks: on a terminal, when the
//! engine is docker, the deploy user cannot open the socket and joining its
//! group would let it. The deploy user is added only on an explicit yes;
//! otherwise, and with no terminal, init prints the same two choices
//! `riku doctor` and a container deploy print, and changes nothing. Any
//! other engine, or a socket the deploy user already reaches, gets no
//! question.
//!
//! Security: the group is the one that owns the socket, as the system
//! resolves its gid, never a name riku assumes. It reaches the group tool
//! only through [`add_to_group`], which refuses anything but a plain name, with a
//! fixed argv and no shell. The engine program comes from the deploy
//! user's `engine.program`, read as that user, or the fixed list on root's
//! `PATH`, and is only looked up, never run.

use std::io::{self, BufRead, Write};
use std::os::unix::fs::{MetadataExt, PermissionsExt};

use nix::unistd::{Gid, Group};
use riku_container::engine::detect;

use crate::deploy::containers::SOCKET_CHOICE;
use crate::util::display;

use super::deploy_user::DeployUser;
use super::init_router::deploy_settings;
use super::router_choice::interactive;
use super::socket_group::{add_to_group, is_account_name};
use super::supervisor_binary::{service_main_pid, UnitScope};
use super::system_service::systemctl_ok;

const DOCKER: &str = "docker";
const DOCKER_SOCKET: &str = "/var/run/docker.sock";
const ROOT_GID: u32 = 0;

/// Which engine init found. Only docker gets the group question.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EngineKind {
    Docker,
    Other,
}

/// Whether the deploy user can open the engine's socket.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum SocketAccess {
    /// No socket at the path: the daemon is not running, nothing to judge.
    Missing,
    Reachable,
    /// The deploy user cannot open it. `group` is the socket's group when
    /// joining it would grant access, `None` for a root owned group, one
    /// that does not resolve, or no group write bit.
    Refused {
        group: Option<String>,
    },
}

/// What init does about the socket.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Step {
    Nothing,
    /// Ask whether to add the deploy user to this group.
    Ask(String),
    /// Print the choices `riku doctor` prints and change nothing.
    Explain,
    /// Docker is installed but its daemon is not running.
    NotRunning,
}

pub(super) fn engine_kind(binary_name: &str) -> EngineKind {
    if binary_name == DOCKER {
        EngineKind::Docker
    } else {
        EngineKind::Other
    }
}

/// The step for `kind` and `access`; `tty` is whether init can ask.
pub(super) fn step(kind: EngineKind, access: &SocketAccess, tty: bool) -> Step {
    match (kind, access) {
        (EngineKind::Docker, SocketAccess::Refused { group: Some(group) }) if tty => {
            Step::Ask(group.clone())
        }
        (EngineKind::Docker, SocketAccess::Refused { .. }) => Step::Explain,
        (EngineKind::Docker, SocketAccess::Missing) => Step::NotRunning,
        _ => Step::Nothing,
    }
}

/// Whether the operator's answer is a yes. Anything else, an empty line
/// included, is the default no.
pub(super) fn accepted(answer: &str) -> bool {
    matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}

pub(super) fn question(group: &str, user: &str) -> String {
    format!(
        "Add {user} to the {group} group so it can run container apps? \
         This gives the {user} user root-equivalent access on this host. [y/N] "
    )
}

/// Whether `uid` may write a file owned by `owner` with `mode`; `in_group`
/// is whether `uid` belongs to the file's group. Opening a unix socket
/// takes write permission.
pub(super) fn may_write(owner: u32, mode: u32, uid: u32, in_group: bool) -> bool {
    if uid == owner {
        mode & 0o200 != 0
    } else if in_group {
        mode & 0o020 != 0
    } else {
        mode & 0o002 != 0
    }
}

/// The socket's group, named `name`, when joining it grants the socket:
/// not root's group, resolved to a plain name, with the group write bit.
pub(super) fn joinable_group(gid: u32, name: Option<&str>, mode: u32) -> Option<String> {
    match name {
        Some(name) if gid != ROOT_GID && mode & 0o020 != 0 && is_account_name(name) => {
            Some(name.to_string())
        }
        _ => None,
    }
}

fn group_name(gid: u32) -> Option<Group> {
    Group::from_gid(Gid::from_raw(gid)).ok().flatten()
}

fn socket_access(user: &DeployUser) -> SocketAccess {
    let meta = match std::fs::metadata(DOCKER_SOCKET) {
        Ok(meta) => meta,
        Err(_) => return SocketAccess::Missing,
    };
    let mode = meta.permissions().mode();
    let group = group_name(meta.gid());
    let in_group = meta.gid() == user.gid()
        || group
            .as_ref()
            .is_some_and(|group| group.mem.iter().any(|member| member == &user.name));
    if may_write(meta.uid(), mode, user.uid(), in_group) {
        return SocketAccess::Reachable;
    }
    SocketAccess::Refused {
        group: joinable_group(meta.gid(), group.as_ref().map(|g| g.name.as_str()), mode),
    }
}

/// The engine init would drive, by the deploy user's `engine.program` or
/// the fixed list on `PATH`. `None` when there is none or it cannot be told.
fn found_engine(user: &DeployUser) -> Option<EngineKind> {
    let program = match deploy_settings(user) {
        Ok(settings) => settings.and_then(|settings| settings.engine.program),
        Err(e) => {
            tracing::debug!("cannot read the engine setting: {e:#}");
            return None;
        }
    };
    let search_path = std::env::var_os("PATH").unwrap_or_default();
    match detect(program.as_deref(), &search_path) {
        Ok(binary) => Some(engine_kind(&binary.binary_name)),
        Err(_) => None,
    }
}

fn ask(group: &str, user: &str) -> bool {
    print!("{}", question(group, user));
    let _ = io::stdout().flush();
    let mut answer = String::new();
    match io::stdin().lock().read_line(&mut answer) {
        Ok(_) => accepted(&answer),
        Err(_) => false,
    }
}

/// Restart a running supervisor so it starts with the new group. systemd
/// sets a service's groups when it starts it.
fn restart_supervisor(group: &str, no_systemd: bool) {
    let running = matches!(service_main_pid(UnitScope::System), Ok(Some(_)));
    if no_systemd || !running {
        display::detail(&format!(
            "The riku supervisor gets the {group} group the next time it starts"
        ));
        return;
    }
    if systemctl_ok(&["restart", "riku"]) {
        display::step_done("Service", &format!("restarted (joined {group})"));
    } else {
        display::warn(&format!(
            "systemctl restart riku failed; restart it so the supervisor joins {group}"
        ));
    }
}

fn join(group: &str, user: &DeployUser, no_systemd: bool) {
    // The error names the command to run by hand, or the NixOS lines.
    if let Err(e) = add_to_group(group, &user.name) {
        display::warn(&format!("{e:#}"));
        return;
    }
    display::step_done("Container engine", &format!("{} joined {group}", user.name));
    restart_supervisor(group, no_systemd);
}

/// Offer to add the deploy user to the group that owns docker's socket.
/// Never fails init: a refused or failed change is printed and init goes on.
/// Runs after the service is installed, so a restart picks up the group.
pub(super) fn offer_engine_group(user: &DeployUser, no_systemd: bool) {
    let Some(kind) = found_engine(user) else {
        return;
    };
    match step(kind, &socket_access(user), interactive()) {
        Step::Nothing => {}
        Step::Explain => display::warn(SOCKET_CHOICE),
        Step::NotRunning => display::warn(
            "docker is installed but not running; start it, then rerun riku init to set up access",
        ),
        Step::Ask(group) => {
            if ask(&group, &user.name) {
                join(&group, user, no_systemd);
            } else {
                display::warn(SOCKET_CHOICE);
            }
        }
    }
}

#[cfg(test)]
#[path = "engine_group_tests.rs"]
mod tests;
