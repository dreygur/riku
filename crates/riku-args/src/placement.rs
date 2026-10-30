//! Where each top-level command runs: on this machine only, on the server
//! only, or on the selected server when one is configured. `client_rule` is
//! the same question for the laptop only `rk` binary.

const CONTAINER: &str = "container";
const DASHBOARD: &str = "dashboard";

/// Execution site of a top-level command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    /// Always runs on the machine the user typed it on.
    Local,
    /// Only meaningful on the server itself (setup, daemons, git plumbing).
    Server,
    /// Runs on the selected server, or here when no server is selected.
    Forwarded,
}

/// Returns the placement of a top-level subcommand, by its clap name.
///
/// `None` for names that are not riku commands, including clap's own `help`.
pub fn placement_of(name: &str) -> Option<Placement> {
    match name {
        "server" | "quickstart" | "container" | "plugin" => Some(Placement::Local),
        "init" | "supervisor" | "dashboard" | "git-hook" | "git-receive-pack"
        | "git-upload-pack" | "scp" | "plugin-emit" | "__ns-shim" | "__dump-state" => {
            Some(Placement::Server)
        }
        "apps" | "ps" | "logs" | "stats" | "config" | "deploy" | "restart" | "stop" | "destroy"
        | "rollback" | "run" | "backup" | "restore" | "addon" | "doctor" | "update" | "agent"
        | "install-plugins" | "plugins" | "hook" => Some(Placement::Forwarded),
        _ => None,
    }
}

/// Where a command that `rk` refuses has to be run instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Elsewhere {
    /// Only the riku on the server can run it.
    OnServer,
    /// Only the full riku binary can run it.
    FullBinary,
}

/// How `rk` treats a top-level command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientRule {
    /// Runs on the laptop.
    RunsHere,
    /// Goes to the selected server.
    Forwards,
    /// Not part of `rk`; the payload says where to run it instead.
    Refused(Elsewhere),
}

/// The one rule for what `rk` runs, forwards and refuses, by clap name.
///
/// `dashboard` is server only for riku but runs here in its remote form, and
/// `container` is local for riku but needs the full binary. Names that are
/// not riku commands forward, as the server reports them.
pub fn client_rule(name: &str) -> ClientRule {
    if name == CONTAINER {
        return ClientRule::Refused(Elsewhere::FullBinary);
    }
    if name == DASHBOARD {
        return ClientRule::RunsHere;
    }
    match placement_of(name) {
        Some(Placement::Local) => ClientRule::RunsHere,
        Some(Placement::Server) => ClientRule::Refused(Elsewhere::OnServer),
        Some(Placement::Forwarded) | None => ClientRule::Forwards,
    }
}

#[cfg(test)]
#[path = "placement_tests.rs"]
mod tests;
