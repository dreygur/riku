//! Where each top-level command runs: on this machine only, on the server
//! only, or on the selected server when one is configured.

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

#[cfg(test)]
#[path = "placement_tests.rs"]
mod tests;
