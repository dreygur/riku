//! The `plugins scaffold`, `keygen` and `sign` commands, which an author runs
//! on their own machine. `riku` and `rk` both call them from here, so the
//! behaviour and output are one implementation.
//!
//! Security model: the bundle name is validated before it becomes a path, and
//! scaffolding refuses to write over an existing path. The secret key is
//! written with mode 0600 and never printed. Signing covers the entry file
//! only, as described in `riku-bundle`.

mod keys;
mod scaffold;
mod skeleton;

pub use keys::{cmd_plugins_keygen, cmd_plugins_sign};
pub use scaffold::cmd_plugins_scaffold;

/// The binary running an authoring command, which decides how the hints it
/// prints name the commands to run next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Program {
    Riku,
    Rk,
}

impl Program {
    /// The binary name as the user types it.
    pub const fn name(self) -> &'static str {
        match self {
            Program::Riku => "riku",
            Program::Rk => "rk",
        }
    }

    /// What to do between editing a bundle and installing it. Empty for
    /// `riku`, which installs from the local path. `rk` installs on the
    /// server, which takes a local path on the server or a git URL, so the
    /// bundle has to get there first.
    fn before_install(self) -> &'static str {
        match self {
            Program::Riku => "",
            Program::Rk => " push the bundle to a git repository or copy it to the server,",
        }
    }

    /// The install command for the bundle at `path` on this machine.
    fn install_command(self, path: &str) -> String {
        match self {
            Program::Riku => format!("riku plugins install {path}"),
            Program::Rk => "rk -s <server> plugins install <git-url or path on server>".to_string(),
        }
    }

    /// The command that trusts a public key. It runs on the server, so `rk`
    /// has to name one.
    fn trust_add(self) -> &'static str {
        match self {
            Program::Riku => "riku plugins trust add",
            Program::Rk => "rk -s <server> plugins trust add",
        }
    }
}
